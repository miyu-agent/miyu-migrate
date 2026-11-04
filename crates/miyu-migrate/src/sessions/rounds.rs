//! 一轮里的工具来回（细则第一节第 11 条「工具」）：有 `tool_flow`（v20 起）的照它；没有、只有展示转录 `replay_journal` 的照转录
//! 拼（工具的参数、输出被旧版截过，报告里数出来）；提问 `question_exchanges` 写成一次 `ask_user` 调用和结果。

use serde::Deserialize;
use serde_json::{Value, json};

/// 一次来回：她说的、想的，和这一次调的工具。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Round {
    /// 说的。
    #[serde(default)]
    pub assistant_content: String,
    /// 想的。
    #[serde(default)]
    pub assistant_reasoning: Option<String>,
    /// 调的工具。
    #[serde(default)]
    pub calls: Vec<Call>,
}

/// 一次调用。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Call {
    /// 旧编号：工具出的图、溢出照它对。
    #[serde(default)]
    pub id: String,
    /// 工具名。
    #[serde(default)]
    pub name: String,
    /// 参数原串。
    #[serde(default)]
    pub arguments: String,
    /// 模型当时看到的输出。
    #[serde(default)]
    pub output: String,
    /// 开始（Unix 毫秒）。
    #[serde(default)]
    pub started_ms: Option<u64>,
    /// 结束（Unix 毫秒）。
    #[serde(default)]
    pub finished_ms: Option<u64>,
    /// 结果来过没有（照转录拼时用）。
    #[serde(skip)]
    pub answered: bool,
}

/// `tool_flow` 的原文读成几次来回；读不懂的是空的。
pub fn from_flow(flow: Option<&str>) -> Vec<Round> {
    flow.and_then(|flow| serde_json::from_str(flow).ok())
        .unwrap_or_default()
}

/// 展示转录拼成几次来回：字、思考接在这一次后面；调用排进这一次；结果对上这一次里头一个还没结果的同名调用。这一次的调用都有
/// 了结果以后，再说话、再调用的是下一次。
pub fn from_replay(journal: Option<&str>) -> Vec<Round> {
    let entries: Vec<Value> = journal
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default();
    let mut rounds = Vec::new();
    let mut current = Round::default();
    for entry in entries {
        let kind = entry["kind"].as_str().unwrap_or_default();
        let text = entry["text"].as_str().unwrap_or_default();
        let done = !current.calls.is_empty() && current.calls.iter().all(|call| call.answered);
        match kind {
            "text" | "reasoning" | "tool_call" if done => {
                rounds.push(std::mem::take(&mut current));
            }
            _ => {}
        }
        match kind {
            "text" => current.assistant_content.push_str(text),
            "reasoning" => current
                .assistant_reasoning
                .get_or_insert_with(String::new)
                .push_str(text),
            "tool_call" => current.calls.push(Call {
                name: entry["name"].as_str().unwrap_or_default().to_string(),
                arguments: entry["arguments"].as_str().unwrap_or_default().to_string(),
                ..Call::default()
            }),
            "tool_result" => {
                let name = entry["name"].as_str().unwrap_or_default();
                if let Some(call) = current
                    .calls
                    .iter_mut()
                    .find(|call| !call.answered && call.name == name)
                {
                    call.answered = true;
                    call.output = entry["output"].as_str().unwrap_or_default().to_string();
                }
            }
            _ => {}
        }
    }
    if !current.assistant_content.is_empty()
        || current.assistant_reasoning.is_some()
        || !current.calls.is_empty()
    {
        rounds.push(current);
    }
    rounds
}

/// 一次提问写成的一次来回：调 `ask_user`，参数是问的题，结果是「问：…／答：…」。
pub fn question(exchange: &Value) -> Round {
    let questions = exchange["questions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let answers = exchange["answers"].as_array().cloned().unwrap_or_default();
    let mut said = Vec::new();
    for (n, asked) in questions.iter().enumerate() {
        let answer: Vec<&str> = answers
            .get(n)
            .and_then(Value::as_array)
            .map(|all| all.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        said.push(format!(
            "问：{}\n答：{}",
            asked["question"].as_str().unwrap_or_default(),
            answer.join("、")
        ));
    }
    Round {
        calls: vec![Call {
            name: "ask_user".to_string(),
            arguments: json!({"questions": questions}).to_string(),
            output: said.join("\n\n"),
            answered: true,
            ..Call::default()
        }],
        ..Round::default()
    }
}

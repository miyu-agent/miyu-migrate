//! `miyu migrate <旧目录>`：核心照包清单转交过来（`miyu-migrate <旧目录>`），状态目录由 `MIYU_STATE_DIR` 给。

use std::path::PathBuf;
use std::process::ExitCode;

use miyu_migrate::run::{self, Plan};

const HELP: &str = "用法：miyu migrate <旧目录>

把旧版 Miyu（0.7.x）的数据目录迁进这一版。先停掉旧版，再把旧目录挪开（例如 mv ~/.miyu ~/.miyu-0.7），然后跑这一条。
旧目录会整个打成压缩包留着；迁完看报告，确认没问题以后再删旧目录。
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let old = match args.as_slice() {
        [flag] if flag == "-h" || flag == "--help" => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        [old] => PathBuf::from(old),
        _ => {
            eprint!("{HELP}");
            return ExitCode::from(2);
        }
    };
    let Some(state) = std::env::var_os("MIYU_STATE_DIR").map(PathBuf::from) else {
        eprintln!("run this as `miyu migrate <old folder>`: MIYU_STATE_DIR is not set");
        return ExitCode::from(2);
    };
    let plan = Plan {
        old,
        state,
        at: jiff::Timestamp::now(),
    };
    match run::run(&plan) {
        Ok(done) => {
            println!(
                "报告：{}\n给 agent 的：{}",
                done.human.display(),
                done.agent.display()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("miyu migrate: {error}");
            ExitCode::FAILURE
        }
    }
}

//! 连新核心（细则第十四节）：人在终端里跑的包命令照 miyu-im 的写法连，读 `run/socket`、出示本机令牌，核心认成本机的人。
//! 迁移逻辑只拿 [`Core`] 这个一问一答的口子，测试换成假的或者进程里的真核心。

use std::fmt;
use std::sync::Arc;

use serde_json::Value;

use miyu_client::CoreCommand;
use miyu_client::open;
use miyu_client::places::DataRoot;

/// 握手报的头。
pub const HEAD: &str = "miyu-migrate";

/// 一问一答。
pub trait Core {
    /// 调 `method`，交回 `result`。
    ///
    /// # Errors
    ///
    /// 核心拒了、连接断了：[`Said`]。
    fn call(&mut self, method: &str, params: Value) -> Result<Value, Said>;
}

/// 核心拒了（或者没回）：代码和原话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    /// 拒绝的代码（`persona_invalid` 这样），连接的毛病是 `io`、`no_answer`、`disconnected`。
    pub code: String,
    /// 原话。
    pub message: String,
}

impl fmt::Display for Said {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Said {}

/// 连着的真核心：自己带一个运行时，一问一答在里面跑完。
pub struct Live {
    runtime: tokio::runtime::Runtime,
    core: open::Core,
    /// 请求编号的前缀：进程号加起来的时刻，同一个编号核心照旧回应不执行，所以每次跑都换。
    prefix: String,
    next: u64,
}

impl Live {
    /// 连 `root` 的核心，没在跑就照 `start` 拉起。
    ///
    /// # Errors
    ///
    /// 起不了运行时、连不上、握手被拒：[`Said`]。
    pub fn connect(
        root: &DataRoot,
        start: impl Fn() -> std::process::Command + Send + Sync + 'static,
    ) -> Result<Live, Said> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| Said {
                code: "io".to_string(),
                message: error.to_string(),
            })?;
        let start: CoreCommand = Arc::new(start);
        let core = runtime
            .block_on(open::Core::connect(root, &start, HEAD))
            .map_err(said)?;
        let prefix = format!(
            "migrate-{}-{}",
            std::process::id(),
            jiff::Timestamp::now().as_millisecond()
        );
        Ok(Live {
            runtime,
            core,
            prefix,
            next: 0,
        })
    }
}

impl Core for Live {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, Said> {
        self.next += 1;
        let id = format!("{}-{}", self.prefix, self.next);
        self.runtime
            .block_on(self.core.call(&id, method, params))
            .map_err(said)
    }
}

/// 核心的拒绝换成 [`Said`]：代码是它的 `data.reason`（连接的毛病是固定的词）。
fn said(refused: open::Refused) -> Said {
    Said {
        code: refused.reason,
        message: refused.message,
    }
}

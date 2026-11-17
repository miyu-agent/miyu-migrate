//! `miyu migrate <旧目录>`：核心照包清单转交过来（`miyu-migrate <旧目录>`），状态目录由 `MIYU_STATE_DIR` 给，`miyu` 程序在
//! 哪由 `MIYU_PROGRAM` 给（没给的照名字找）。照 miyu-im 的写法连核心：核心没在跑就照 `miyu core` 拉起。

use std::path::PathBuf;
use std::process::ExitCode;

use miyu_client::places::{DataRoot, Env};
use miyu_migrate::core::Live;
use miyu_migrate::run::{self, Plan};
use miyu_migrate::words::{Language, cli};

fn main() -> ExitCode {
    // 连上核心以前照系统语言说，连上以后照核心握手回的（施工 MG-21）。
    let words = cli::words(Language::system());
    let args: Vec<String> = std::env::args().skip(1).collect();
    let old = match args.as_slice() {
        [flag] if flag == "-h" || flag == "--help" => {
            print!("{}", words.help);
            return ExitCode::SUCCESS;
        }
        [old] => PathBuf::from(old),
        _ => {
            eprint!("{}", words.help);
            return ExitCode::from(2);
        }
    };
    let Some(state) = std::env::var_os("MIYU_STATE_DIR").map(PathBuf::from) else {
        eprintln!("miyu migrate: {}", words.no_state_dir);
        return ExitCode::from(2);
    };
    let root = match DataRoot::locate(&Env::current()) {
        Ok(root) => root,
        Err(error) => {
            eprintln!("miyu migrate: {error}");
            return ExitCode::FAILURE;
        }
    };
    let program =
        std::env::var_os("MIYU_PROGRAM").map_or_else(|| PathBuf::from("miyu"), PathBuf::from);
    let records = miyu_migrate::records::Importer::program(program.clone());
    let mut core = match Live::connect(&root, move || {
        let mut core = std::process::Command::new(&program);
        core.arg("core");
        core
    }) {
        Ok(core) => core,
        Err(said) => {
            eprintln!("miyu migrate: {}", (words.cannot_reach)(&said.to_string()));
            return ExitCode::FAILURE;
        }
    };
    let language = Language::from_core(core.language());
    let plan = Plan {
        old,
        state,
        at: jiff::Timestamp::now(),
        root: root.clone(),
        home: std::env::home_dir(),
        records,
        space: run::free_space,
        language,
    };
    match run::run(&plan, &mut core) {
        Ok(done) => {
            println!("{}", (cli::words(language).done)(&done.human, &done.agent));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("miyu migrate: {}", error.say(language));
            ExitCode::FAILURE
        }
    }
}

mod ast_sig;
mod compare;
mod corpus;
mod lex;
mod report;

pub use corpus::Config;

pub fn run(config: &Config) -> Result<report::Summary, String> {
    corpus::run(config)
}

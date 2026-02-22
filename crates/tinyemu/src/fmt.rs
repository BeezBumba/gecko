use colored::Colorize;
use disasm::tokenizer::{self, AsmToken};

pub fn colorize(tok: &AsmToken<'_>) -> String {
    match tok {
        AsmToken::Mnemonic(s) => s.bold().cyan().to_string(),
        AsmToken::Gpr(n) => format!("r{n}").yellow().to_string(),
        AsmToken::Fpr(n) => format!("f{n}").magenta().to_string(),
        AsmToken::CrField(n) => format!("cr{n}").green().to_string(),
        AsmToken::Spr(s) => s.green().bold().to_string(),
        AsmToken::ImmSigned(v) => format!("{v}").blue().to_string(),
        AsmToken::ImmUnsigned(v) => format!("{v}").blue().to_string(),
        AsmToken::ImmHex(v) if *v < 0 => format!("-0x{:X}", -v).blue().to_string(),
        AsmToken::ImmHex(v) => format!("0x{v:X}").blue().to_string(),
        AsmToken::Displacement(v) => format!("{v}").blue().to_string(),
        AsmToken::BranchTarget(s) => s.bright_red().to_string(),
        AsmToken::Punct(_) | AsmToken::Text(_) => tok.to_string(),
    }
}

pub fn colorize_instr(instr: &disasm::gekko::GekkoInstruction) -> String {
    let text = format!("{}", instr);
    let tokens = tokenizer::tokenize(&text);
    tokens
        .into_iter()
        .map(|t| colorize(&t))
        .collect::<Vec<_>>()
        .join("")
}

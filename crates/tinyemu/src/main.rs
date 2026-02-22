mod fmt;
mod snaptshot;

use colored::Colorize;
use disasm::gekko::GekkoInstruction;
use snaptshot::CpuSnapshot;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("Usage: debugger <path_to_rom>");
    let is_debug = std::env::args().any(|arg| arg == "--debug");

    tracing_subscriber::fmt()
        .without_time()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let mut gekko = gekko::gekko::Gekko::new(&path);
    let mut prev_snapshot = CpuSnapshot::from_cpu(&gekko.cpu);

    loop {
        let instr = GekkoInstruction::decode(gekko.mmu.virt_slice(gekko.cpu.pc, 4))
            .expect("failed to decode instruction")
            .0;

        if is_debug {
            dbg!(&instr);
        }

        let refs = fmt::gpr_refs(&instr);
        let comment = fmt::reg_comment(&prev_snapshot.gprs, &refs);

        let prefix = format!(
            "{}: {}",
            format!("{:08X}", gekko.cpu.pc).bold(),
            fmt::colorize_instr(&instr)
        );
        const COMMENT_COL: usize = 50;
        let pad = COMMENT_COL.saturating_sub(fmt::visible_len(&prefix));

        if comment.is_empty() {
            println!("{}", prefix);
        } else {
            println!("{}{}{}", prefix, " ".repeat(pad), comment);
        }

        gekko.run_until_event();
        let curr_snapshot = CpuSnapshot::from_cpu(&gekko.cpu);

        if is_debug {
            dump_registers(&curr_snapshot, &prev_snapshot);
        }

        prev_snapshot = curr_snapshot;
    }
}

fn dump_registers(curr: &CpuSnapshot, prev: &CpuSnapshot) {
    let fmt_reg = |label: &str, val: u32, prev_val: u32| -> String {
        let value = format!("{:08X}", val);
        if val != prev_val {
            format!("{} {} ", label.yellow().bold(), value.bright_red().bold())
        } else {
            format!("{} {} ", label.dimmed(), value.dimmed())
        }
    };

    for row in 0..8 {
        let line: String = (0..4)
            .map(|col| {
                let i = row * 4 + col;
                fmt_reg(&format!("r{:<2}", i), curr.gprs[i], prev.gprs[i])
            })
            .collect();
        println!("{}", line.trim_end());
    }

    println!(
        "{}",
        format!(
            "{}{}",
            fmt_reg("lr ", curr.lr, prev.lr),
            fmt_reg("ctr", curr.ctr, prev.ctr)
        )
        .trim_end()
    );

    println!();
}

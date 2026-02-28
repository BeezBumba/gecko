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
    let mut current_addr = gekko.cpu.pc;
    let mut is_busyloop = false;

    loop {
        if !is_busyloop {
            let instr = GekkoInstruction::decode(gekko.mmu.virt_slice(gekko.cpu.pc, 4))
                .unwrap_or_else(|| {
                    dump_registers(&prev_snapshot, &prev_snapshot);
                    dump_memory(&gekko.mmu, gekko.cpu.read_gpr(1));

                    panic!("Failed to decode instruction at {:08X}", gekko.cpu.pc)
                })
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
        }

        gekko.run_until_event();

        let curr_snapshot = CpuSnapshot::from_cpu(&gekko.cpu);

        if current_addr == gekko.cpu.pc {
            if !is_busyloop {
                println!("{}", "Busyloop detected!".bright_red().bold());
                is_busyloop = true;
            }
        } else {
            is_busyloop = false;
        }

        current_addr = gekko.cpu.pc;

        if is_debug && !is_busyloop {
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

    let cr_fields = [
        ("cr0", curr.cr.cr0(), prev.cr.cr0()),
        ("cr1", curr.cr.cr1(), prev.cr.cr1()),
        ("cr2", curr.cr.cr2(), prev.cr.cr2()),
        ("cr3", curr.cr.cr3(), prev.cr.cr3()),
        ("cr4", curr.cr.cr4(), prev.cr.cr4()),
        ("cr5", curr.cr.cr5(), prev.cr.cr5()),
        ("cr6", curr.cr.cr6(), prev.cr.cr6()),
        ("cr7", curr.cr.cr7(), prev.cr.cr7()),
    ];

    let fmt_cr_field = |label: &str,
                        val: gekko::cpu::condition::ConditionField,
                        prev_val: gekko::cpu::condition::ConditionField| {
        let flags = format!(
            "{}{}{}{}",
            if val.lt() { "L" } else { "·" },
            if val.gt() { "G" } else { "·" },
            if val.eq() { "Z" } else { "·" },
            if val.so() { "O" } else { "·" },
        );
        let text = format!("{}[{}] ", label, flags);
        if val.raw() != prev_val.raw() {
            format!("{}", text.bright_red().bold())
        } else {
            format!("{}", text.dimmed())
        }
    };

    let cr_line: String = cr_fields
        .iter()
        .map(|(label, val, prev_val)| fmt_cr_field(label, *val, *prev_val))
        .collect();
    println!("{}", cr_line.trim_end());

    println!();
}

fn dump_memory(mmu: &gekko::mmu::Mmu, addr: u32) {
    let aligned_addr = addr & !0xF;
    let start = aligned_addr.wrapping_sub(0x40);
    let data = mmu.virt_slice(start, 0x80);

    for (i, line) in data.chunks(16).enumerate() {
        let line_addr = start.wrapping_add((i as u32) * 16);
        let hex = line
            .chunks(4)
            .map(|chunk| {
                let word = u32::from_be_bytes(chunk.try_into().unwrap());
                format!("{:08X}", word)
            })
            .collect::<Vec<_>>()
            .join(" ");

        println!("{} {}", format!("{:08X}:", line_addr).blue().bold(), hex);
    }
}

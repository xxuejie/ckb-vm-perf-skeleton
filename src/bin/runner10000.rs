use ckb_vm::cost_model::estimate_cycles;
use ckb_vm::registers::{A0, A7};
use ckb_vm::{Bytes, CoreMachine, Memory, Register, SupportMachine, Syscalls};

pub struct DebugSyscall {}

impl<Mac: SupportMachine> Syscalls<Mac> for DebugSyscall {
    fn initialize(&mut self, _machine: &mut Mac) -> Result<(), ckb_vm::error::Error> {
        Ok(())
    }

    fn ecall(&mut self, machine: &mut Mac) -> Result<bool, ckb_vm::error::Error> {
        let code = &machine.registers()[A7];
        if code.to_i32() != 2177 {
            return Ok(false);
        }

        let mut addr = machine.registers()[A0].to_u64();
        let mut buffer = Vec::new();

        loop {
            let byte = machine
                .memory_mut()
                .load8(&Mac::REG::from_u64(addr))?
                .to_u8();
            if byte == 0 {
                break;
            }
            buffer.push(byte);
            addr += 1;
        }

        let s = String::from_utf8(buffer).unwrap();
        println!("{:?}", s);

        Ok(true)
    }
}

fn main_asm(code: Bytes, args: Vec<Bytes>) -> Result<(), Box<dyn std::error::Error>> {
    use std::time::{Duration, SystemTime};
    cpuprofiler::PROFILER
        .lock()
        .unwrap()
        .start("./run.profile")
        .unwrap();

    let mut last_result = None;
    let times = 10000;

    let mut runtime = Duration::new(0, 0);
    for i in 0..times {
        let a = SystemTime::now();
        let result = {
            let asm_core = ckb_vm::machine::asm::AsmCoreMachine::new(
                ckb_vm::ISA_IMC | ckb_vm::ISA_B | ckb_vm::ISA_MOP | ckb_vm::ISA_A,
                ckb_vm::machine::VERSION2,
                u64::MAX,
            );
            let core = ckb_vm::DefaultMachineBuilder::new(asm_core)
                .instruction_cycle_func(Box::new(estimate_cycles))
                .syscall(Box::new(DebugSyscall {}))
                .build();
            let mut machine = ckb_vm::machine::asm::AsmMachine::new(core);
            machine.load_program(&code, &args)?;
            let exit = machine.run();
            let cycles = machine.machine.cycles();
            (
                exit,
                cycles,
                machine.machine.registers()[ckb_vm::registers::A1],
            )
        };
        let b = SystemTime::now();
        runtime = runtime.checked_add(b.duration_since(a).unwrap()).unwrap();

        if let Some(last_result) = last_result {
            assert_eq!(last_result, result);
        }
        last_result = Some(result);

        if i % 1000 == 0 {
            println!("Step: {}", i);
        }
    }
    let (exit, cycles, a1) = last_result.unwrap();
    println!("asm exit={:?} cycles={:?} r[a1]={:?}", exit, cycles, a1,);
    println!("Average runtime: {:?}", runtime / times);
    std::process::exit(exit? as i32);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let code = std::fs::read(&args[1])?.into();
    let riscv_args: Vec<Bytes> = if args.len() > 2 {
        (&args[2..]).into_iter().map(|s| s.clone().into()).collect()
    } else {
        Vec::new()
    };
    main_asm(code, riscv_args)?;
    Ok(())
}

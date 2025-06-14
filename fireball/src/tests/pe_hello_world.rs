use crate::binary::pe::Pe;
use crate::{
    core::{Address, FireRaw, RelationType},
    prelude::*,
};

fn get_binary() -> &'static [u8] {
    include_bytes!("../../tests/resources/hello_world.exe")
}

#[test]
fn pe_hello_world() {
    test_init();
    let binary = get_binary();
    let pe = Pe::from_binary(binary.to_vec()).unwrap();
    dbg!(pe);
}

#[test]
fn pe_hello_world_entry() {
    test_init();
    let binary = get_binary();
    let pe = Pe::from_binary(binary.to_vec()).unwrap();
    let gl = goblin::pe::PE::parse(binary).unwrap();
    let sections = pe.get_sections();
    let pe_entry = pe.entry();
    let gl_entry = Address::from_virtual_address(&sections, gl.entry as u64);
    assert_eq!(pe_entry, &gl_entry);
}

#[test]
fn pe_hello_world_entry_parse() {
    test_init();
    let binary = get_binary();
    let pe = Pe::from_binary(binary.to_vec()).unwrap();
    let gl = goblin::pe::PE::parse(binary).unwrap();

    let sections = pe.get_sections();

    // Verify assembly parsing based on virtual address
    let entry_of_virtual_address = Address::from_virtual_address(&sections, gl.entry as u64);
    let insts_by_virtual_address = pe
        .parse_assem_range(&entry_of_virtual_address, 0x60)
        .unwrap();

    // Compute file offset for entry point (as of 2022-10-19: 0x725)
    let mut entry_of_file_offset = 0;
    for section in gl.sections {
        if section.virtual_address as u64 <= gl.entry as u64
            && gl.entry as u64 <= (section.virtual_address + section.virtual_size) as u64
        {
            entry_of_file_offset = gl.entry as u64 - section.virtual_address as u64
                + section.pointer_to_raw_data as u64;
            break;
        }
    }

    // Verify assembly parsing based on file offset
    let entry_of_file_offset = Address::from_file_offset(&sections, entry_of_file_offset);
    let insts_by_file_offset = pe.parse_assem_range(&entry_of_file_offset, 0x60).unwrap();

    // Check if both results match
    for (left, right) in insts_by_virtual_address
        .iter()
        .zip(insts_by_file_offset.iter())
    {
        assert_eq!(left, right);
    }
}

#[test]
fn pe_hello_world_detect_block_entry() {
    test_init();
    let binary = get_binary();
    let pe = Pe::from_binary(binary.to_vec()).unwrap();
    let entry = pe.entry();
    let block = pe.generate_block_from_address(entry);

    assert_eq!(&block.get_section().unwrap().name, ".text");
    assert_eq!(block.get_start_address(), entry);
    assert_eq!(block.get_block_size(), Some(&33)); // verified with debugger
}

#[test]
fn pe_hello_world_detect_block_etc() {
    test_init();
    let binary = get_binary();
    let pe = Pe::from_binary(binary.to_vec()).unwrap();
    let gl = goblin::pe::PE::parse(binary).unwrap();
    let sections = pe.get_sections();
    let entry = Address::from_virtual_address(&sections, gl.entry as u64);
    for offset in std::iter::once(-6).chain(2..=7) {
        info!("Parsing at offset {}", offset);
        let address = if offset < 0 {
            &entry - (-offset) as u64
        } else {
            &entry + offset as u64
        };
        let block = pe.generate_block_from_address(&address);
        assert_eq!(&block.get_section().unwrap().name, ".text");
        assert_eq!(*block.get_start_address(), address);
        assert_ne!(block.get_block_size(), Some(&0));
    }
}

#[test]
fn pe_hello_world_block_relation() {
    test_init();
    let binary = get_binary();
    let pe = Pe::from_binary(binary.to_vec()).unwrap();

    /* Validate parsing and relation creation for the entry block */
    let entry = pe.entry();
    pe.generate_block_from_address(entry);
    let blocks = pe.get_blocks();
    let entry_block = blocks.get_by_start_address(entry);
    assert!(entry_block.is_some());
    let entry_block = entry_block.unwrap();
    let entry_block_id = entry_block.get_id();
    let entry_connected_to = entry_block.get_connected_to();
    assert_eq!(entry_connected_to.len(), 2);
    for connected_to in entry_connected_to.iter() {
        assert!(matches!(
            connected_to.relation_type(),
            &RelationType::Call | &RelationType::Halt
        ));
    }

    /* Validate block creation for the entry's target */
    let to_address = entry_connected_to
        .iter()
        .find(|x| x.relation_type() == &RelationType::Call)
        .and_then(|x| x.to())
        .unwrap();
    pe.generate_block_from_address(&to_address);
    let blocks = pe.get_blocks();
    let to_block = blocks.get_by_start_address(&to_address);
    assert!(to_block.is_some());
    let to_block = to_block.unwrap();
    // check connected from
    let to_connected_from = to_block.get_connected_from();
    assert_eq!(to_connected_from.len(), 1);
    assert_eq!(to_connected_from[0].from(), entry_block_id);
    // check connected to
    let to_connected_to = to_block.get_connected_to();
    assert_eq!(to_connected_to.len(), 2);
    assert_eq!(
        to_connected_to
            .iter()
            .find(|x| x.relation_type() == &RelationType::Call)
            .and_then(|x| x.to())
            .map(|x| x.get_virtual_address())
            .unwrap(),
        37216
    );
}

#[test]
fn pe_hello_world_decom_block() {
    test_init();
    let binary = get_binary();
    let pe = Pe::from_binary(binary.to_vec()).unwrap();

    /* Start decompilation from the entry point */
    let result = pe.analyze_from_entry();
    assert!(result.is_ok(), "Decompilation failed");
    let result = result.unwrap();
    let entry = pe.entry();
    let blocks = pe.get_blocks();
    let block = blocks.get_by_start_address(entry);
    assert!(block.is_some(), "No data for decompiled block");
    let block = block.unwrap();
    assert_eq!(&block, &result, "Decompiled result does not match block");
    let ir = block.get_ir();
    assert!(ir.is_some(), "IR data not generated during decompilation");
    let ir = ir.as_ref().unwrap();
    assert!(
        ir.data_access.is_some(),
        "Data access not analyzed during decompilation"
    );
    assert!(
        ir.known_datatypes.is_some(),
        "Data types not analyzed during decompilation"
    );
    assert!(
        ir.variables.is_some(),
        "Variables not analyzed during decompilation"
    );
}

#[test]
fn pe_hello_world_analyze_variables() {
    test_init();
    let binary = get_binary();
    let pe = Pe::from_binary(binary.to_vec()).unwrap();

    /* Start decompilation from the entry point */
    let result = pe.analyze_from_entry();
    assert!(result.is_ok(), "Decompilation failed");
    let block = result.unwrap();
    let ir = block.get_ir();
    assert!(ir.is_some(), "IR data not generated during decompilation");
    let ir = ir.as_ref().unwrap();
    let analyzed_variables = ir.variables.as_ref().unwrap();
    assert_ne!(analyzed_variables.len(), 0);
    for variable in analyzed_variables {
        println!("{:?}", variable);
    }
}

#[test]
fn pe_hello_world_print_statements() {
    test_init();
    let binary = get_binary();
    let pe = Pe::from_binary(binary.to_vec()).unwrap();

    /* Start decompilation from the entry point */
    let result = pe.analyze_from_entry();
    assert!(result.is_ok(), "Decompilation failed");
    let block = result.unwrap();
    let ir = block.get_ir();
    assert!(ir.is_some(), "IR data not generated during decompilation");
    let ir = ir.as_ref().unwrap();
    for (ir, instruction) in ir.ir().iter().zip(ir.instructions().iter()) {
        println!("{} {}", ir.address.get_virtual_address(), instruction);
        for statement in ir.statements.as_ref().unwrap().iter() {
            println!("{}", statement);
        }
    }
}

#[test]
fn pe_hello_world_print_assem_entry() {
    test_init();
    let binary = get_binary();
    let pe = Pe::from_binary(binary.to_vec()).unwrap();
    let entry = pe.entry();
    let insts = pe.parse_assem_range(entry, 0x60).unwrap();
    for inst in insts {
        /*
        push rbp
        mov rbp, rsp
        sub rsp, 0x30
        mov dword ptr [rbp - 4], 0xff
        mov rax, qword ptr [rip + 0xa675]
        mov dword ptr [rax], 0
        call 0x1154
        mov dword ptr [rbp - 4], eax
        nop
        nop
        mov eax, dword ptr [rbp - 4]
        add rsp, 0x30
        pop rbp
        ret
        push rbp
        mov rbp, rsp
        sub rsp, 0xe0
        mov qword ptr [rbp - 8], 0
        mov dword ptr [rbp - 0xc], 0
        lea rax, [rbp - 0xc0]
        mov r8d, 0x68
        mov edx, 0
        mov rcx, rax
         */
        println!("{}", inst);
    }
}

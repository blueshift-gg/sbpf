use sbpf_assembler::ast::AST;
use sbpf_assembler::astnode::{ASTNode, ROData};
use sbpf_assembler::instruction::Instruction;
use sbpf_assembler::lexer::{ImmediateValue, Token};
use sbpf_assembler::opcode::Opcode;
use sbpf_assembler::parser::ParseResult;

use object::{File, Object, ObjectSection, ObjectSymbol};
use object::RelocationTarget::Symbol;

use anyhow::Result;
use std::collections::HashMap;

pub fn parse_bytecode(bytes: &[u8]) -> Result<ParseResult, String> {
    let mut ast = AST::new();

    let obj = File::parse(bytes).map_err(|e| e.to_string())?;

    // only handle symbols in the .rodata section for now
    let ro_section = obj.section_by_name(".rodata").unwrap();
    let mut rodata_table = HashMap::new();
    let mut rodata_offset = 0;
    for symbol in obj.symbols() {
        if symbol.section_index() == Some(ro_section.index()) && symbol.size() > 0 {
            let mut bytes = Vec::new();
            for i in 0..symbol.size() {
                bytes.push(ImmediateValue::Int(
                    ro_section.data().unwrap()[(symbol.address() + i) as usize] as i64));
            }
            ast.rodata_nodes.push(ASTNode::ROData {
                rodata: ROData {
                    name: symbol.name().unwrap().to_string(),
                    args: vec![Token::Directive(String::from("byte"), 0..1) //
                            , Token::VectorLiteral(bytes.clone(), 0..1)],
                    span: 0..1,
                },
                offset: rodata_offset,
            });
            rodata_table.insert(symbol.address(), symbol.name().unwrap().to_string());
            rodata_offset += symbol.size();
        }
    }
    ast.set_rodata_size(rodata_offset);

    for section in obj.sections() {
        if section.name() == Ok(".text") {
            // parse text section and build instruction nodes
            // lddw takes 16 bytes, other instructions take 8 bytes
            let mut offset = 0;
            while offset < section.data().unwrap().len() {
                let node_len = match Opcode::from_u8(section.data().unwrap()[offset]) {
                    Some(Opcode::Lddw) => 16,
                    _ => 8,
                };
                let node = &section.data().unwrap()[offset..offset + node_len];
                ast.nodes.push(ASTNode::Instruction {
                    instruction: Instruction::from_bytes(node).unwrap(),
                    offset: offset as u64,
                });
                offset += node_len;
            }
            
            // handle relocations
            for rel in section.relocations() {
                // only handle relocations for symbols in the .rodata section for now
                let symbol = match rel.1.target() {
                    Symbol(sym) => Some(obj.symbol_by_index(sym).unwrap()), 
                    _ => None
                };
                
                if symbol.unwrap().section_index() == Some(ro_section.index()) {
                    // addend is not explicit in the relocation entry, but implicitly encoded
                    // as the immediate value of the instruction
                    let addend = //
                        match ast.get_instruction_at_offset(rel.0 as u64).unwrap()
                            .operands.last().unwrap().clone() {
                                Token::ImmediateValue(ImmediateValue::Int(val), _) => val,
                                _ => 0 
                        };

                    // Replace the immediate value with the rodata label
                    let ro_label = rodata_table.get(&(addend as u64)).unwrap();
                    let ro_label_name = ro_label.clone();
                    let node: &mut Instruction = ast.get_instruction_at_offset(rel.0 as u64).unwrap();
                    let last_idx = node.operands.len() - 1;
                    node.operands[last_idx] = Token::Identifier(ro_label_name, 0..1);
                }
            }
            ast.set_text_size(section.size());
        }
    }

    let parse_result = ast.build_program();
    if let Ok(parse_result) = parse_result {
        Ok(parse_result)
    } else {
        // TODO: handle errors
        Err("Failed to build program".to_string())
    }
}

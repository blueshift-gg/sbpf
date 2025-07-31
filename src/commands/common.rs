use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct SolanaConfig {
    pub releases_dir: String,
    pub active_release_dir: String,
}

pub const PROGRAM: &str = r#".globl entrypoint
entrypoint:
  lddw r1, message
  lddw r2, 14
  call sol_log_
  exit
.rodata
  message: .ascii "Hello, Solana!"
"#;

pub const DEFAULT_LINKER: &str = r#"PHDRS
{
  text    PT_LOAD    ;
  data    PT_LOAD    ;
  dynamic PT_DYNAMIC ;
}

SECTIONS
{
  . = SIZEOF_HEADERS;
  .text    : { *(.text*)   } : text    
  .rodata  : { *(.rodata*) } : text    
  .dynamic : { *(.dynamic) } : dynamic 
  .dynsym  : { *(.dynsym)  } : data    
  /DISCARD/ : {
    *(.eh_frame*)
    *(.gnu.hash*)
    *(.hash*)    
    *(.comment)  
    *(.symtab)   
    *(.strtab)   
  }
}

ENTRY (entrypoint)"#;

pub const README: &str = r#"# default_project_name

Created with [sbpf](https://github.com/deanmlittle/sbpf)"#;

pub const GITIGNORE: &str = r#"build/**/*
deploy/**/*
node_modules
.sbpf
.DS_Store
.vscode
keypair.json
package-lock.json
test-ledger
yarn.lock"#;

pub const PACKAGE_JSON: &str = r#"{
  "name": "default_project_name",
  "description": "Created with sBPF",
  "version": "1.0.0",
  "main": "index.js",
  "license": "MIT",
  "scripts": {
    "test": "KEYPAIR=$(solana config get | grep Keypair | cut -b 15-) && cross-env SIGNER=$(cat $KEYPAIR) mocha --import=tsx tests/**/*.ts"
  },
  "dependencies": {
    "@solana/web3.js": "^1.91.8"
  },
  "devDependencies": {
    "cross-env": "^7.0.3",
    "@types/chai": "^4.3.16",
    "@types/mocha": "^10.0.6",
    "chai": "^5.1.1",
    "mocha": "^10.4.0",
    "tsx": "^4.11.0"
  }
}
"#;

pub const TS_TESTS: &str = r#"
import { Connection, Keypair, Transaction, TransactionInstruction } from "@solana/web3.js"
import programSeed from "../deploy/default_project_name-keypair.json"

const programKeypair = Keypair.fromSecretKey(new Uint8Array(programSeed))
const program = programKeypair.publicKey
const signerSeed = JSON.parse(process.env.SIGNER!)
const signer = Keypair.fromSecretKey(new Uint8Array(signerSeed))

const connection = new Connection("http://127.0.0.1:8899", {
    commitment: "confirmed"
})

const confirm = async (signature: string): Promise<string> => {
    const block = await connection.getLatestBlockhash()
    await connection.confirmTransaction({
        signature,
        ...block,
    })
    return signature
}

const log = async (signature: string): Promise<string> => {
    console.log(`Transaction successful! https://explorer.solana.com/tx/${signature}?cluster=custom&customUrl=http%3A%2F%2Flocalhost%3A8899`)
    return signature
}

const signAndSend = async(tx: Transaction): Promise<string> => {
    const block = await connection.getLatestBlockhash()
    tx.recentBlockhash = block.blockhash
    tx.lastValidBlockHeight = block.lastValidBlockHeight
    const signature = await connection.sendTransaction(tx, [signer])
    return signature
}

describe('hello solana tests', () => {
    it('Logs out "Hello, Solana!"', async () => {
        const tx = new Transaction()
        tx.instructions.push(
            new TransactionInstruction({
            keys: [{
                pubkey: signer.publicKey,
                isSigner: true,
                isWritable: true
            }],
            programId: program
        }))
        await signAndSend(tx).then(confirm).then(log);
    });
});
"#;

pub const TSCONFIG: &str = r#"
{
    "compilerOptions": {
        "target": "es6",
        "module": "commonjs",
        "strict": true,
        "esModuleInterop": true,
        "resolveJsonModule": true,
        "skipLibCheck": true,
        "forceConsistentCasingInFileNames": true
    }
}
"#;

pub const CARGO_TOML: &str = r#"[package]
name = "default_project_name"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"
thiserror = "1.0"

[dev-dependencies]
mollusk-svm = "0.4.1"
solana-sdk = "2.2.1"

[lib]
crate-type = ["cdylib"]

[profile.release]
overflow-checks = true
lto = true
codegen-units = 1
opt-level = 3

[profile.release.build-override]
opt-level = 3
codegen-units = 1

[features]
test-sbf = []
"#;

pub const RUST_TESTS: &str = r#"#[cfg(test)]
mod tests {
    use mollusk_svm::{result::Check, Mollusk};
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::instruction::Instruction;

    #[test]
    fn test_hello_world() {
        let program_id_keypair_bytes = std::fs::read("deploy/default_project_name-keypair.json").unwrap()
            [..32]
            .try_into()
            .expect("slice with incorrect length");
        let program_id = Pubkey::new_from_array(program_id_keypair_bytes);

        let instruction = Instruction::new_with_bytes(
            program_id,
            &[],
            vec![]
        );

        let mollusk = Mollusk::new(&program_id, "deploy/default_project_name");

        let result = mollusk.process_and_validate_instruction(
            &instruction,
            &[],
            &[Check::success()]
        );
        assert!(!result.program_result.is_err());
    }
}"#;

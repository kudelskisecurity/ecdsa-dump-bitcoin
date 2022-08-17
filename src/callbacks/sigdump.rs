use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{PathBuf};

use clap::{App, Arg, ArgMatches, SubCommand};
use ecdsa::Signature;
use p256::{
    NistP256, NonZeroScalar,
};
use bitcoin_explorer::{BitcoinDB, Txid, Transaction, FromHex};

use blockchain::proto::script::ScriptPattern::ScriptSig;
use blockchain::proto::ToRaw;
use blockchain::proto::tx::TxOutpoint;
use blockchain::proto::varuint::VarUint;

use crate::blockchain::parser::types::CoinType;
use crate::blockchain::proto::block::Block;
use crate::blockchain::proto::tx::{EvaluatedTx, EvaluatedTxOut, TxInput};
use crate::blockchain::proto::Hashed;
use crate::callbacks::Callback;
use crate::common::utils;
use crate::errors::OpResult;

/// Dumps the whole blockchain into csv files
pub struct SigDump {
    // Each structure gets stored in a separate csv file
    dump_folder: PathBuf,
    sig_writer: BufWriter<File>,

    start_height: u64,
    end_height: u64,
    tx_count: u64,
    in_count: u64,
    out_count: u64,
    blocks_count: u64,
    db: BitcoinDB,
}

impl SigDump {
    fn create_writer(cap: usize, path: PathBuf) -> OpResult<BufWriter<File>> {
        Ok(BufWriter::with_capacity(cap, File::create(&path)?))
    }

    fn get_previous_outputs(&mut self, previous_txid: Vec<u8>) -> Option<Vec<Vec<u8>>> {
        let txid_str = utils::arr_to_hex_swapped(&previous_txid);
        let txid = Txid::from_hex(&txid_str).ok()?;
        let tx: Transaction = self.db.get_transaction(&txid).ok()?;

        let mut outputs: Vec<Vec<u8>> = Vec::new();

        for output in tx.output {
            outputs.push(output.script_pubkey.to_bytes());
        }

        Some(outputs)
    }
}


impl Callback for SigDump {
    fn build_subcommand<'a, 'b>() -> App<'a, 'b>
        where
            Self: Sized,
    {
        SubCommand::with_name("sigdump")
            .about("Dumps signatures to CSV file")
            .version("0.1")
            .author("Nils Amiet <nils.amiet@kudelskisecurity.com>")
            .arg(
                Arg::with_name("dump-folder")
                    .help("Folder to store csv files")
                    .index(1)
                    .required(true),
            )
            .arg(
                Arg::with_name("bitcoin-folder")
                    .help("Path to the .bitcoin folder")
                    .index(2)
                    .required(true),
            )
    }

    fn new(matches: &ArgMatches) -> OpResult<Self>
        where
            Self: Sized,
    {
        let dump_folder = &PathBuf::from(matches.value_of("dump-folder").unwrap());
        if !dump_folder.exists() {
            eprintln!("Dump folder {} does not exist. Attempting to create it...", dump_folder.display());
            match fs::create_dir_all(dump_folder) {
                Ok(_) => {
                    eprintln!("Successfully created dump folder.")
                }
                Err(_) => {
                    panic!("Failed to create dump folder: {}", dump_folder.display());
                }
            }
        }
        let bitcoin_folder = &PathBuf::from(matches.value_of("bitcoin-folder").unwrap());
        let cap = 4000000;
        let db = BitcoinDB::new(bitcoin_folder, true).expect("Failure to create Bitcoin DB");
        let cb = SigDump {
            dump_folder: PathBuf::from(dump_folder),
            sig_writer: SigDump::create_writer(cap, dump_folder.join("signatures.csv.tmp"))?,
            start_height: 0,
            end_height: 0,
            tx_count: 0,
            in_count: 0,
            out_count: 0,
            blocks_count: 0,
            db,
        };
        Ok(cb)
    }

    fn on_start(&mut self, _: &CoinType, block_height: u64) -> OpResult<()> {
        self.start_height = block_height;
        info!(target: "callback", "Using `sigdump` with dump folder: {} ...", &self.dump_folder.display());
        Ok(())
    }

    fn on_block(&mut self, block: &Block, _block_height: u64) -> OpResult<()> {
        self.blocks_count += 1;
        let block_time: u32 = block.header.value.timestamp;

        for tx in &block.txs {
            let txid_str = utils::arr_to_hex_swapped(&tx.hash);

            let mut message_to_be_signed: Vec<u8> = Vec::new();
            let version = tx.value.version.to_le_bytes();
            let tx_in_count = tx.value.in_count.clone().to_bytes();
            message_to_be_signed.extend_from_slice(&version);
            message_to_be_signed.extend_from_slice(&tx_in_count);

            // serialize inputs
            let mut input_index = 0;
            for input in &tx.value.inputs {
                match &input.script.pattern {
                    ScriptSig(sig, pubkey) => {
                        // actually parse signature
                        let only_sig = &sig[..sig.len() - 1];
                        let hash_type: u8 = sig[sig.len() - 1];
                        match Signature::<NistP256>::from_der(only_sig) {
                            Ok(esig) => {
                                let r: NonZeroScalar = esig.r();
                                let s: NonZeroScalar = esig.s();

                                // make a copy of message to be signed
                                let mut tbs_message = message_to_be_signed.clone();

                                // build modified inputs and add them to message to be signed
                                let mut raw_input_index = 0;
                                for raw_input in &tx.value.inputs {
                                    let mut r_input = TxInput {
                                        outpoint: TxOutpoint {
                                            txid: raw_input.input.outpoint.txid,
                                            index: raw_input.input.outpoint.index,
                                        },
                                        script_len: 0u8.into(),
                                        script_sig: [].to_vec(),
                                        seq_no: raw_input.input.seq_no,
                                    };

                                    if raw_input_index == input_index {
                                        // replace script_len and script_sig with script from previous output
                                        let previous_output_txid = raw_input.input.outpoint.txid.to_vec();
                                        let previous_output_index = raw_input.input.outpoint.index as usize;

                                        let previous_outputs = self.get_previous_outputs(previous_output_txid);
                                        let empty_script = vec![];
                                        let subscript = match &previous_outputs {
                                            Some(prev_outs) => &prev_outs[previous_output_index],
                                            None => {
                                                &empty_script
                                            }
                                        };

                                        let script_len = VarUint::from(subscript.len() as u8);
                                        r_input.script_sig = subscript.clone();
                                        r_input.script_len = script_len;
                                    }

                                    // add modified input to message to be signed
                                    let mut input_bytes = Vec::with_capacity(36 + 5 + r_input.script_len.value as usize + 4);
                                    let outpoint_txid = r_input.outpoint.txid;
                                    let outpoint_index = r_input.outpoint.index.to_le_bytes();
                                    let script_len = r_input.script_len.to_bytes();
                                    let script_sig = r_input.script_sig.clone();
                                    let sequence = r_input.seq_no.to_le_bytes();
                                    input_bytes.extend_from_slice(&outpoint_txid);
                                    input_bytes.extend_from_slice(&outpoint_index);
                                    input_bytes.extend_from_slice(&script_len);
                                    input_bytes.extend_from_slice(&script_sig);
                                    input_bytes.extend_from_slice(&sequence);

                                    tbs_message.extend_from_slice(&input_bytes);
                                    raw_input_index += 1;
                                }

                                // add number of outputs to message to be signed
                                let tx_out_count = tx.value.out_count.clone().to_bytes();
                                tbs_message.extend_from_slice(&tx_out_count);

                                // add outputs to message to be signed
                                for output in &tx.value.outputs {
                                    let output_value = output.out.value.to_le_bytes();
                                    let script_len = output.out.script_len.to_bytes();
                                    let pubkey_script = output.out.script_pubkey.clone();
                                    let mut output_bytes = Vec::with_capacity(8 + 5 + output.out.script_len.value as usize);
                                    output_bytes.extend_from_slice(&output_value);
                                    output_bytes.extend_from_slice(&script_len);
                                    output_bytes.extend_from_slice(&pubkey_script);

                                    tbs_message.extend_from_slice(&output_bytes);
                                }

                                // finalize message hash
                                let locktime = tx.value.locktime.to_le_bytes().to_vec();
                                let hash_code_type = (hash_type as u32).to_le_bytes().to_vec();
                                tbs_message.extend_from_slice(&locktime);
                                tbs_message.extend_from_slice(&hash_code_type);

                                // double sha256
                                let mut message_hash = utils::sha256(&tbs_message);
                                message_hash = utils::sha256(&message_hash);

                                let message_hash_str = utils::arr_to_hex(&message_hash);

                                self.sig_writer
                                    .write_all(
                                        input.as_csv(
                                            r, s, pubkey, &txid_str, message_hash_str, block_time,
                                        ).as_bytes())?;
                            }
                            Err(_e) => {}
                        }
                    }
                    _ => {}
                }
                input_index += 1;
            } // end for input
            self.in_count += tx.value.in_count.value;
            self.out_count += tx.value.out_count.value;
        } // end for tx
        self.tx_count += block.tx_count.value;
        Ok(())
    }

    fn on_complete(&mut self, block_height: u64) -> OpResult<()> {
        self.end_height = block_height;

        // Keep in sync with c'tor
        for f in &["signatures"] {
            // Rename temp files
            fs::rename(
                self.dump_folder.as_path().join(format!("{}.csv.tmp", f)),
                self.dump_folder.as_path().join(format!(
                    "{}-{}-{}.csv",
                    f, self.start_height, self.end_height
                )),
            )?;
        }

        info!(target: "callback", "Done.\nDumped all {} blocks:\n\
                                   \t-> transactions: {:9}\n\
                                   \t-> inputs:       {:9}\n\
                                   \t-> outputs:      {:9}",
             self.end_height, self.tx_count, self.in_count, self.out_count);
        Ok(())
    }
}

impl Block {
    #[inline]
    pub fn as_csv(&self, block_height: u64) -> String {
        // (@hash, height, version, blocksize, @hashPrev, @hashMerkleRoot, nTime, nBits, nNonce)
        format!(
            "{};{};{};{};{};{};{};{};{}\n",
            &utils::arr_to_hex_swapped(&self.header.hash),
            &block_height,
            &self.header.value.version,
            &self.size,
            &utils::arr_to_hex_swapped(&self.header.value.prev_hash),
            &utils::arr_to_hex_swapped(&self.header.value.merkle_root),
            &self.header.value.timestamp,
            &self.header.value.bits,
            &self.header.value.nonce
        )
    }
}

impl Hashed<EvaluatedTx> {
    #[inline]
    pub fn as_csv(&self, block_hash: &str) -> String {
        // (@txid, @hashBlock, version, lockTime)
        format!(
            "{};{};{};{}\n",
            &utils::arr_to_hex_swapped(&self.hash),
            &block_hash,
            &self.value.version,
            &self.value.locktime
        )
    }
}

impl TxInput {
    #[inline]
    pub fn as_csv(&self, txid: &str) -> String {
        // (@txid, @hashPrevOut, indexPrevOut, scriptSig, sequence)
        format!(
            "{};{};{};{};{}\n",
            &txid,
            &utils::arr_to_hex_swapped(&self.outpoint.txid),
            &self.outpoint.index,
            &utils::arr_to_hex(&self.script_sig),
            &self.seq_no
        )
    }
}

impl EvaluatedTxOut {
    #[inline]
    pub fn as_csv(&self, txid: &str, index: u32) -> String {
        let address = match self.script.address.clone() {
            Some(address) => address,
            None => {
                debug!(target: "sigdump", "Unable to evaluate address for utxo in txid: {} ({})", txid, self.script.pattern);
                String::new()
            }
        };

        // (@txid, indexOut, value, @scriptPubKey, address)
        format!(
            "{};{};{};{};{}\n",
            &txid,
            &index,
            &self.out.value,
            &utils::arr_to_hex(&self.out.script_pubkey),
            &address
        )
    }
}

use std::fmt;

use p256::NonZeroScalar;
use utils::arr_to_hex;

use crate::blockchain::proto::script;
use crate::blockchain::proto::varuint::VarUint;
use crate::blockchain::proto::ToRaw;
use crate::common::utils;

pub struct RawTx {
    pub version: u32,
    pub in_count: VarUint,
    pub inputs: Vec<TxInput>,
    pub out_count: VarUint,
    pub outputs: Vec<TxOutput>,
    pub locktime: u32,
    pub version_id: u8,
}

/// Simple transaction struct
/// Please note: The txid is not stored here. See Hashed.
pub struct EvaluatedTx {
    pub version: u32,
    pub in_count: VarUint,
    pub inputs: Vec<EvaluatedTxIn>,
    pub out_count: VarUint,
    pub outputs: Vec<EvaluatedTxOut>,
    pub locktime: u32,
}

impl EvaluatedTx {
    pub fn new(
        version: u32,
        in_count: VarUint,
        inputs: Vec<TxInput>,
        out_count: VarUint,
        outputs: Vec<TxOutput>,
        locktime: u32,
        version_id: u8,
    ) -> Self {
        // Evaluate and wrap all outputs to process them later
        let outputs = outputs
            .into_iter()
            .map(|o| EvaluatedTxOut::eval_script(o, version_id))
            .collect();
        // also evaluate TxInputs
        let inputs = inputs
            .into_iter()
            .map(|i| EvaluatedTxIn::eval_script(i, version_id))
            .collect();
        EvaluatedTx {
            version,
            in_count,
            inputs,
            out_count,
            outputs,
            locktime,
        }
    }

    #[inline]
    pub fn is_coinbase(&self) -> bool {
        if self.in_count.value == 1 {
            let input = self.inputs.first().unwrap();
            return input.input.outpoint.txid == [0u8; 32] && input.input.outpoint.index == 0xFFFFFFFF;
        }
        false
    }
}

impl fmt::Debug for EvaluatedTx {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Tx")
            .field("version", &self.version)
            .field("in_count", &self.in_count)
            .field("out_count", &self.out_count)
            .field("locktime", &self.locktime)
            .finish()
    }
}

impl From<RawTx> for EvaluatedTx {
    fn from(tx: RawTx) -> Self {
        Self::new(
            tx.version,
            tx.in_count,
            tx.inputs,
            tx.out_count,
            tx.outputs,
            tx.locktime,
            tx.version_id,
        )
    }
}

impl ToRaw for EvaluatedTx {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes =
            Vec::with_capacity((4 + self.in_count.value + self.out_count.value + 4) as usize);

        // Serialize version
        bytes.extend_from_slice(&self.version.to_le_bytes());
        // Serialize all TxInputs
        bytes.extend_from_slice(&self.in_count.to_bytes());
        for i in &self.inputs {
            bytes.extend_from_slice(&i.input.to_bytes());
        }
        // Serialize all TxOutputs
        bytes.extend_from_slice(&self.out_count.to_bytes());
        for o in &self.outputs {
            bytes.extend_from_slice(&o.out.to_bytes());
        }
        // Serialize locktime
        bytes.extend_from_slice(&self.locktime.to_le_bytes());
        bytes
    }
}

/// TxOutpoint references an existing transaction output
#[derive(PartialEq, Eq, Hash)]
pub struct TxOutpoint {
    pub txid: [u8; 32],
    pub index: u32, // 0-based offset within tx
}

impl TxOutpoint {
    pub fn new(txid: [u8; 32], index: u32) -> Self {
        Self { txid, index }
    }
}

impl ToRaw for TxOutpoint {
    #[inline]
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32 + 4);
        bytes.extend_from_slice(&self.txid);
        bytes.extend_from_slice(&self.index.to_le_bytes());
        bytes
    }
}

impl fmt::Debug for TxOutpoint {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("TxOutpoint")
            .field("txid", &utils::arr_to_hex_swapped(&self.txid))
            .field("index", &self.index)
            .finish()
    }
}

/// Holds TxInput informations
pub struct TxInput {
    pub outpoint: TxOutpoint,
    pub script_len: VarUint,
    pub script_sig: Vec<u8>,
    pub seq_no: u32,
}

impl ToRaw for TxInput {
    #[inline]
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(36 + 5 + self.script_len.value as usize + 4);
        bytes.extend_from_slice(&self.outpoint.to_bytes());
        bytes.extend_from_slice(&self.script_len.to_bytes());
        bytes.extend_from_slice(&self.script_sig);
        bytes.extend_from_slice(&self.seq_no.to_le_bytes());
        bytes
    }
}

impl fmt::Debug for TxInput {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("TxInput")
            .field("outpoint", &self.outpoint)
            .field("script_len", &self.script_len)
            .field("script_sig", &self.script_sig)
            .field("seq_no", &self.seq_no)
            .finish()
    }
}

/// Evaluates scriptSig and wraps TxInput
pub struct EvaluatedTxIn {
    pub script: script::EvaluatedScript,
    pub input: TxInput,
}

impl EvaluatedTxIn {
    pub fn eval_script(input: TxInput, version_id: u8) -> EvaluatedTxIn {
        EvaluatedTxIn {
            script: script::eval_from_bytes(&input.script_sig, version_id),
            input,
        }
    }

    pub fn as_csv(&self, r: NonZeroScalar, s: NonZeroScalar,
                  pubkey: &Vec<u8>, txid: &str,
                  message_hash_str: String, block_time: u32) -> String {
        // (@txid, @hashPrevOut, indexPrevOut, scriptSig, sequence)
        format!(
            "{:x};{:x};{};{};{};{}\n",
            r,
            s,
            arr_to_hex(&pubkey),
            txid,
            message_hash_str,
            block_time
        )
    }
}

/// Evaluates script_pubkey and wraps TxOutput
pub struct EvaluatedTxOut {
    pub script: script::EvaluatedScript,
    pub out: TxOutput,
}

impl EvaluatedTxOut {
    #[inline]
    pub fn eval_script(out: TxOutput, version_id: u8) -> EvaluatedTxOut {
        EvaluatedTxOut {
            script: script::eval_from_bytes(&out.script_pubkey, version_id),
            out,
        }
    }
}

/// Holds TxOutput informations
pub struct TxOutput {
    pub value: u64,
    pub script_len: VarUint,
    pub script_pubkey: Vec<u8>,
}

impl ToRaw for TxOutput {
    #[inline]
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + 5 + self.script_len.value as usize);
        bytes.extend_from_slice(&self.value.to_le_bytes());
        bytes.extend_from_slice(&self.script_len.to_bytes());
        bytes.extend_from_slice(&self.script_pubkey);
        bytes
    }
}

impl fmt::Debug for TxOutput {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("TxOutput")
            .field("value", &self.value)
            .field("script_len", &self.script_len)
            .field("script_pubkey", &utils::arr_to_hex(&self.script_pubkey))
            .finish()
    }
}

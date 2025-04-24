use super::*;
use serde::{Serialize, Deserialize};
use std::fmt::{self, Debug, Formatter};
use ed25519_dalek::{Keypair, PublicKey as DalekPublicKey, Signature as DalekSignature, Signer, Verifier};
use rand::rngs::OsRng;
use crate::ledger::lib::now;

pub type TxHash = Vec<u8>;
pub type PublicKey = Vec<u8>;
pub type Signature = Vec<u8>;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum TransactionType {
    Transfer,
    Data,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TransactionData {
    pub sender: PublicKey,
    pub receiver: Option<PublicKey>,
    pub timestamp: u128,
    pub tx_type: TransactionType,
    pub amount : Option<u64>,
    pub data: Option<String>,
    pub nonce: u64,
    pub fee: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub data: TransactionData,
    pub signature: Signature,
    pub tx_hash: TxHash,
}

impl Debug for Transaction {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "Transaction[{}]: from {} at: {} type: {:?}",
            &hex::encode(&self.tx_hash),
            &hex::encode(&self.data.sender),
            &self.data.timestamp,
            &self.data.tx_type
        )
    }
}

impl Hashable for Transaction {
    fn bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];
        let data_bytes = serde_json::to_vec(&self.data).unwrap_or_default();
        bytes.extend(data_bytes);
        bytes.extend(&self.signature);
        bytes
    }
}

impl Transaction {
    pub fn new_data(
        sender: PublicKey,
        receiver: Option<PublicKey>,
        tx_type: TransactionType,
        amount: Option<u64>,
        data: Option<String>,
        nonce: u64,
        fee: u64,
    ) -> TransactionData {
        TransactionData {
            sender,
            receiver,
            timestamp: now(),
            tx_type,
            amount,
            data,
            nonce,
            fee,
        }
    }

    // Assinar a transação
    pub fn sign(tx_data: &TransactionData, key_pair: &Keypair) -> Signature {
        let data_bytes = serde_json::to_vec(tx_data).unwrap_or_default();
        key_pair.sign(&data_bytes).to_bytes().to_vec()
    }

    pub fn create_signed(tx_data: TransactionData, key_pair: &Keypair) -> Self {
        let signature = Self::sign(&tx_data, key_pair);
        let mut transaction = Transaction {
            data: tx_data,
            signature,
            tx_hash: vec![0; 32],
        };

        transaction.tx_hash = transaction.hash();
        transaction
    }

    // Verificar assinatura da transação
    pub fn verify(&self) -> bool {
        let data_bytes = serde_json::to_vec(&self.data).unwrap_or_default();

        if let Ok(public_key) = DalekPublicKey::from_bytes(&self.data.sender) {
            if let Ok(signature) = DalekSignature::from_bytes(&self.signature) {
                return public_key.verify(&data_bytes, &signature).is_ok();
            }
        }

        false
    }

    pub fn generate_keypair() -> Keypair {
        let mut csprng = OsRng;
        Keypair::generate(&mut csprng)
    }

    pub fn get_public_key(key_pair: &Keypair) -> Vec<u8> {
        key_pair.public.to_bytes().to_vec()
    }

    pub fn create_transfer(
        key_pair: &Keypair,
        receiver: PublicKey,
        amount: u64,
        nonce: u64,
        fee: u64,
    ) -> Self {
        let sender = Self::get_public_key(key_pair);

        let tx_data = TransactionData {
            sender,
            receiver: Some(receiver),
            timestamp: now(),
            tx_type: TransactionType::Transfer,
            amount: Some(amount),
            data: None,
            nonce,
            fee,
        };

        Self::create_signed(tx_data, key_pair)
    }

    pub fn create_data_tx(
        key_pair: &Keypair,
        data: String,
        nonce: u64,
        fee: u64,
    ) -> Self {
        let sender = Self::get_public_key(key_pair);

        let tx_data = TransactionData {
            sender,
            receiver: None,
            timestamp: now(),
            tx_type: TransactionType::Data,
            amount: None,
            data: Some(data),
            nonce,
            fee,
        };

        Self::create_signed(tx_data, key_pair)
    }
}

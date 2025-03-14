use super::*;
use serde::{Serialize, Deserialize};
use std::fmt::{self, Debug, Formatter};
use ring::signature::{self, Ed25519KeyPair, KeyPair, ED25519};
use rand::Rng;


pub type TxHash = Vec<u8>;
pub type PublicKey = Vec<u8>;
pub type Signature = Vec<u8>;

//Phase 1 Basic
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum TransactionType {
    Transfer,
    Data,
}

// Transaction Data Structure 
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
pub struct Transaction{
    pub data: TransactionData,
    pub signature: Signature,
    pub tx_hash: TxHash,
}

impl Debug for Transaction {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result{
        write!(f, "Transaction[{}]: from {} at: {} type: {:?}",
            &hex::encode(&self.tx_hash),
            &hex::encode(&self.data.sender),
            &self.data.timestamp,
            &self.data.tx_type
        )
    }
}

impl Hashable for Transaction {
    fn bytes(&self) -> Vec<u8>{

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

    //Sign a transaction
    pub fn sign(tx_data: &TransactionData, key_pair: &Ed25519KeyPair) -> Signature{
        let data_bytes = serde_json::to_vec(&tx_data).unwrap_or_default();
        let signature = key_pair.sign(&data_bytes);
        
        signature.as_ref().to_vec()
    }

    pub fn create_signed(tx_data: TransactionData, key_pair:&Ed25519KeyPair) -> Self {
        let signature = Self::sign(&tx_data, key_pair);
        let mut transaction = Transaction{
            data: tx_data,
            signature,
            tx_hash: vec![0;32],
        };

        transaction.tx_hash = transaction.hash();

        transaction
    }

    //Verify the signature of a transaction
    pub fn verify(&self) -> bool {
        let data_bytes = serde_json::to_vec(&self.data).unwrap_or_default();

        match signature::UnparsedPublicKey::new(&ED25519, &self.data.sender){
            key => {
                key.verify(&data_bytes, &self.signature).is_ok()
            }
        }
    }

    pub fn generate_keypair() -> Ed25519KeyPair {
        let mut seed = [0u8; 32];
        let mut rng = rand::rng();
        rng.fill(&mut seed);

        Ed25519KeyPair::from_seed_unchecked(&seed).expect("Failed to generate key pair")
    }

    pub fn get_public_key(key_pair: &Ed25519KeyPair) -> PublicKey {
        key_pair.public_key().as_ref().to_vec()
    }

    pub fn create_transfer(
        key_pair: &Ed25519KeyPair,
        receiver: PublicKey,
        amount: u64,
        nonce: u64,
        fee: u64,
    ) -> Self{
        let sender = Self::get_public_key(key_pair);

        let tx_data = TransactionData{
            sender,
            receiver: Some(receiver),
            timestamp: now(),
            tx_type: TransactionType:: Transfer,
            amount: Some(amount),
            data: None,
            nonce,
            fee,
        };

        Self::create_signed(tx_data, key_pair)
    }

    pub fn create_data_tx(
        key_pair: &Ed25519KeyPair,
        data: String,
        nonce: u64,
        fee: u64,
    ) -> Self {
        let sender = Self::get_public_key(key_pair);

        let tx_data = TransactionData{
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
/*
pub fn transaction_to_json(tx: &Transaction) -> Result<String, serde_json::Error>{
    serde_json::to_string(tx)
}

pub fn json_to_transaction(json: &str) -> Result<Transaction, serde_json::Error> {
    serde_json::from_str(json)
}
*/
// src/ledger/transaction.rs
use super::*;
use serde::{Serialize, Deserialize};
use std::fmt::{self, Debug, Formatter};
use ed25519_dalek::{Keypair, PublicKey as DalekPublicKey, Signature as DalekSignature, Signer, Verifier};
use rand::rngs::OsRng;
use crate::ledger::lib::now;
use std::collections::HashMap;

pub type TxHash = Vec<u8>;
pub type PublicKey = Vec<u8>;
pub type Signature = Vec<u8>;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
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
    pub amount: Option<u64>,
    pub data: Option<String>,
    pub nonce: u64,
    pub fee: u64,
    pub valid_until: Option<u128>, // Transaction expiration time
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub data: TransactionData,
    pub signature: Signature,
    pub tx_hash: TxHash,
}

// Nonce tracker for replay protection
pub struct NonceTracker {
    nonces: HashMap<PublicKey, u64>,
}

impl NonceTracker {
    pub fn new() -> Self {
        NonceTracker {
            nonces: HashMap::new(),
        }
    }

    pub fn validate_and_update(&mut self, sender: &PublicKey, tx_nonce: u64) -> bool {
        let current_nonce = self.nonces.get(sender).cloned().unwrap_or(0);
        
        // Nonce must be exactly current_nonce + 1
        if tx_nonce != current_nonce + 1 {
            return false;
        }
        
        self.nonces.insert(sender.clone(), tx_nonce);
        true
    }

    pub fn get_nonce(&self, sender: &PublicKey) -> u64 {
        self.nonces.get(sender).cloned().unwrap_or(0)
    }
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
    // Helper method to create transaction data
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
            valid_until: Some(now() + 3_600_000), // 1 hour validity
        }
    }

    // Sign a transaction
    pub fn sign(tx_data: &TransactionData, key_pair: &Keypair) -> Signature {
        let data_bytes = serde_json::to_vec(tx_data).unwrap_or_default();
        key_pair.sign(&data_bytes).to_bytes().to_vec()
    }

    // Create and sign a transaction
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

    // Verify transaction signature and validity
    pub fn verify(&self) -> bool {
        // Check if transaction has expired
        if let Some(valid_until) = self.data.valid_until {
            if now() > valid_until {
                return false;
            }
        }

        // Check timestamp is reasonable
        let current_time = now();
        if self.data.timestamp > current_time + 3_600_000 { // 1 hour in future
            return false;
        }

        // Check if timestamp is too old (24 hours)
        if self.data.timestamp < current_time.saturating_sub(86_400_000) {
            return false;
        }

        // Validate transaction-specific rules
        if !self.validate_transaction_specifics() {
            return false;
        }

        // Verify signature
        let data_bytes = serde_json::to_vec(&self.data).unwrap_or_default();

        if let Ok(public_key) = DalekPublicKey::from_bytes(&self.data.sender) {
            if let Ok(signature) = DalekSignature::from_bytes(&self.signature) {
                return public_key.verify(&data_bytes, &signature).is_ok();
            }
        }

        false
    }

    // Validate transaction-specific rules
    fn validate_transaction_specifics(&self) -> bool {
        match self.data.tx_type {
            TransactionType::Transfer => {
                // Must have valid amount and receiver
                if let Some(amount) = self.data.amount {
                    if amount == 0 {
                        return false;
                    }
                    if self.data.receiver.is_none() {
                        return false;
                    }
                    
                    // Check for reasonable maximum transfer amount
                    if amount > 1_000_000_000_000 { // 1 trillion max
                        return false;
                    }
                } else {
                    return false;
                }
            },
            TransactionType::Data => {
                // Must have valid data
                if let Some(ref data) = self.data.data {
                    if data.is_empty() || data.len() > 4096 { // Increased max size for auction data
                        return false;
                    }
                    
                    // Validate data content (no control characters)
                    if data.chars().any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t') {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }
    
        // Make fees optional - accept zero fee for auction transactions
        // For auction transactions, we don't require fees
        if let Some(ref data) = self.data.data {
            if data.starts_with("AUCTION_") {
                return true;
            }
        }
        
        // For non-auction transactions, validate fee
        if self.data.fee > 1_000_000 { // Max fee of 1M
            return false;
        }
    
        true
    }

    // Generate a keypair
    pub fn generate_keypair() -> Keypair {
        let mut csprng = OsRng;
        Keypair::generate(&mut csprng)
    }

    // Get public key from keypair
    pub fn get_public_key(key_pair: &Keypair) -> Vec<u8> {
        key_pair.public.to_bytes().to_vec()
    }

    // Create a transfer transaction
    pub fn create_transfer(
        key_pair: &Keypair,
        receiver: PublicKey,
        amount: u64,
        nonce: u64,
        fee: u64,
    ) -> Result<Self, &'static str> {
        if amount == 0 {
            return Err("Transfer amount cannot be zero");
        }

        if receiver.len() != 32 {
            return Err("Invalid receiver public key length");
        }

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
            valid_until: Some(now() + 3_600_000), // 1 hour validity
        };

        Ok(Self::create_signed(tx_data, key_pair))
    }

    pub fn create_data_tx(
        key_pair: &Keypair,
        data: String,
        nonce: u64,
        fee: u64,
    ) -> Result<Self, &'static str> {
        if data.is_empty() {
            return Err("Data cannot be empty");
        }

        if data.len() > 1024 {
            return Err("Data too large (max 1KB)");
        }

        // Sanitize data
        let sanitized_data = data.chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
            .collect::<String>();

        let sender = Self::get_public_key(key_pair);

        let tx_data = TransactionData {
            sender,
            receiver: None,
            timestamp: now(),
            tx_type: TransactionType::Data,
            amount: None,
            data: Some(sanitized_data),
            nonce,
            fee,
            valid_until: Some(now() + 3_600_000), // 1 hour validity
        };

        Ok(Self::create_signed(tx_data, key_pair))
    }

    // Check if transaction can be applied (for double-spend prevention)
    pub fn can_be_applied(&self, balances: &HashMap<PublicKey, u64>) -> bool {
        // For auction transactions, don't require balance
        if let Some(ref data) = self.data.data {
            if data.starts_with("AUCTION_") {
                return true; // Auction transactions don't require balance
            }
        }
        
        match self.data.tx_type {
            TransactionType::Transfer => {
                if let Some(amount) = self.data.amount {
                    let sender_balance = balances.get(&self.data.sender).unwrap_or(&0);
                    let total_needed = amount + self.data.fee;
                    return *sender_balance >= total_needed;
                }
            },
            TransactionType::Data => {
                let sender_balance = balances.get(&self.data.sender).unwrap_or(&0);
                return *sender_balance >= self.data.fee;
            }
        }
        false
    }
}
use ledgerlib::*;

fn main() {
    println!("Hello, ledger!");

    let block = Block::new(13,now(),vec![0;32],0,"Genesis block!!".to_owned());

    println!("{:?}", &block);
}
 
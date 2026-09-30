use mycryptomoney::chain::Blockchain;
use mycryptomoney::net::listener::start_listener;
use mycryptomoney::net::node::Node;
use mycryptomoney::utxo_set::UTXOSet;
use mycryptomoney::wallet::Wallet;


#[tokio::main]
async fn main() {
    let mut wallet = Wallet::new();
    let mut blockchain = Blockchain::new();
    let mut utxo_set = UTXOSet::new();

    wallet.mine_block(&[], &mut blockchain, &mut utxo_set);
    let balance0 = wallet.get_balance(&utxo_set);
    wallet.mine_block(&[], &mut blockchain, &mut utxo_set);
    let balance1 = wallet.get_balance(&utxo_set);
    println!("Balance after mining 2 blocks: {}", balance1);
    println!("Balance after mine 1 blocks: {}", balance0);
}
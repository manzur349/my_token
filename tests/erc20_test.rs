use ethers::{
    prelude::*,
    providers::{Http, Provider},
    types::{U256, H160, TransactionRequest},
};
use eyre::Result;
use std::sync::Arc;
use std::str::FromStr;

// Test contract ABI
abigen!(
    TestERC20,
    r#"[
        function name() external view returns (string)
        function symbol() external view returns (string)
        function decimals() external view returns (uint8)
        function totalSupply() external view returns (uint256)
        function balanceOf(address account) external view returns (uint256)
        function transfer(address to, uint256 amount) external returns (bool)
        function allowance(address owner, address spender) external view returns (uint256)
        function approve(address spender, uint256 amount) external returns (bool)
        function transferFrom(address from, address to, uint256 amount) external returns (bool)
    ]"#,
);

// Use a single test that runs all the checks sequentially
#[tokio::test]
async fn test_erc20_contract() -> Result<()> {
    // Connect to local Anvil instance
    let provider = Provider::<Http>::try_from("http://localhost:8545")?;
    
    // Use the deployer's private key
    let deployer_key = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    let owner = LocalWallet::from_str(deployer_key)?
        .with_chain_id(31337u64);
    
    let client = Arc::new(SignerMiddleware::new(
        provider.clone(),
        owner.clone(),
    ));
    
    // Update this with your deployed contract address
    let contract_address = H160::from_str("0x5FbDB2315678afecb367f032d93F642f64180aa3").unwrap();
    let contract = TestERC20::new(contract_address, client.clone());
    
    let other_account = LocalWallet::new(&mut rand::thread_rng())
        .with_chain_id(31337u64);
    println!("Other account address: {}", other_account.address());
    
    // Test 1: Initial state
    println!("Testing initial state...");
    assert_eq!(contract.name().call().await?, "MyToken");
    assert_eq!(contract.symbol().call().await?, "MTK");
    assert_eq!(contract.decimals().call().await?, 18);
    
    let expected_supply = U256::from(1000000) * U256::from(10).pow(U256::from(18));
    let total_supply = contract.total_supply().call().await?;
    assert_eq!(total_supply, expected_supply);
    
    let owner_balance = contract.balance_of(owner.address()).call().await?;
    assert_eq!(owner_balance, total_supply);
    
    // Test 2: Transfer
    println!("Testing transfer...");
    let amount = U256::from(100);
    let nonce = client.get_transaction_count(
        owner.address(),
        None
    ).await?;
    
    let tx = contract
        .transfer(other_account.address(), amount)
        .legacy()
        .gas(300000)
        .gas_price(U256::from(2u64 * 1_000_000_000u64))
        .nonce(nonce);
    
    let pending_tx = tx.send().await?;
    let receipt = pending_tx.await?;
    println!("Transfer transaction confirmed: {:?}", receipt.unwrap().transaction_hash);
    
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    let recipient_balance = contract
        .balance_of(other_account.address())
        .call()
        .await?;
    assert_eq!(recipient_balance, amount);
    
    // *** IMPORTANT: Send ETH to other_account to pay for gas ***
    println!("Funding other_account with ETH for gas...");
    let nonce = client.get_transaction_count(
        owner.address(),
        None
    ).await?;
    
    let tx_request = TransactionRequest::new()
        .to(other_account.address())
        .value(U256::from(1000000000000000000u64)) // 1 ETH
        .gas(21000)
        .gas_price(U256::from(2u64 * 1_000_000_000u64))
        .nonce(nonce);
    
    let pending_tx = client.send_transaction(tx_request, None).await?;
    let receipt = pending_tx.await?;
    println!("ETH funding transaction confirmed: {:?}", receipt.unwrap().transaction_hash);
    
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Test 3: Approve and TransferFrom
    println!("Testing approve and transferFrom...");
    let approve_amount = U256::from(200);
    let nonce = client.get_transaction_count(
        owner.address(),
        None
    ).await?;
    
    let approve_tx = contract
        .approve(other_account.address(), approve_amount)
        .legacy()
        .gas(300000)
        .gas_price(U256::from(3u64 * 1_000_000_000u64))
        .nonce(nonce);
    
    let pending_tx = approve_tx.send().await?;
    let receipt = pending_tx.await?;
    println!("Approve transaction confirmed: {:?}", receipt.unwrap().transaction_hash);
    
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    let allowance = contract
        .allowance(owner.address(), other_account.address())
        .call()
        .await?;
    assert_eq!(allowance, approve_amount);
    
    // Now create a client for the other account to execute transferFrom
    let other_client = Arc::new(SignerMiddleware::new(
        provider.clone(),
        other_account.clone(),
    ));
    
    // Check other_account's ETH balance
    let balance = provider.get_balance(other_account.address(), None).await?;
    println!("Other account ETH balance: {}", balance);
    
    let other_contract = TestERC20::new(contract_address, other_client.clone());
    
    let recipient = LocalWallet::new(&mut rand::thread_rng())
        .with_chain_id(31337u64);
    println!("Recipient address: {}", recipient.address());
    
    let transfer_amount = U256::from(150);
    let nonce = other_client.get_transaction_count(
        other_account.address(),
        None
    ).await?;
    
    let transfer_tx = other_contract
        .transfer_from(owner.address(), recipient.address(), transfer_amount)
        .legacy()
        .gas(300000)
        .gas_price(U256::from(4u64 * 1_000_000_000u64))
        .nonce(nonce);
    
    let pending_tx = transfer_tx.send().await?;
    let receipt = pending_tx.await?;
    println!("TransferFrom transaction confirmed: {:?}", receipt.unwrap().transaction_hash);
    
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    let recipient_balance = contract
        .balance_of(recipient.address())
        .call()
        .await?;
    assert_eq!(recipient_balance, transfer_amount);
    
    // Test 4: Insufficient balance - use call() instead of send()
    println!("Testing insufficient balance...");
    let transfer_amount = total_supply + U256::from(1);
    
    // Use call() to check if the transaction would revert
    let call_result = contract
        .transfer(other_account.address(), transfer_amount)
        .call()
        .await;
    
    match call_result {
        Ok(_) => {
            println!("Unexpected success: transfer with insufficient balance should fail");
            panic!("Test failed: transaction with insufficient balance succeeded");
        },
        Err(e) => {
            println!("Expected error occurred: {:?}", e);
            println!("Insufficient balance test passed - transaction correctly failed");
        }
    }
    
    // Test 5: Insufficient allowance - use call() instead of send()
    println!("Testing insufficient allowance...");
    let new_recipient = LocalWallet::new(&mut rand::thread_rng())
        .with_chain_id(31337u64);
    
    let another_account = LocalWallet::new(&mut rand::thread_rng())
        .with_chain_id(31337u64);
    
    // Fund another_account with ETH
    let nonce = client.get_transaction_count(
        owner.address(),
        None
    ).await?;
    
    let tx_request = TransactionRequest::new()
        .to(another_account.address())
        .value(U256::from(1000000000000000000u64)) // 1 ETH
        .gas(21000)
        .gas_price(U256::from(6u64 * 1_000_000_000u64))
        .nonce(nonce);
    
    let pending_tx = client.send_transaction(tx_request, None).await?;
    let receipt = pending_tx.await?;
    println!("Funding transaction confirmed for another_account: {:?}", receipt.unwrap().transaction_hash);
    
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    let another_client = Arc::new(SignerMiddleware::new(
        provider.clone(),
        another_account.clone(),
    ));
    
    let another_contract = TestERC20::new(contract_address, another_client.clone());
    
    // Use call() to check if the transaction would revert
    let call_result = another_contract
        .transfer_from(owner.address(), new_recipient.address(), U256::from(100))
        .call()
        .await;
    
    match call_result {
        Ok(_) => {
            println!("Unexpected success: transferFrom with no allowance should fail");
            panic!("Test failed: transaction with no allowance succeeded");
        },
        Err(e) => {
            println!("Expected error occurred: {:?}", e);
            println!("Insufficient allowance test passed - transaction correctly failed");
        }
    }
    
    println!("All tests completed successfully!");
    Ok(())
}

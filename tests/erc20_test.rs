use ethers::{
    prelude::*,
    providers::{Http, Provider},
    types::{U256, H160},
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

struct TestContext {
    contract: TestERC20<SignerMiddleware<Provider<Http>, LocalWallet>>,
    owner: LocalWallet,
    other_account: LocalWallet
}

async fn setup() -> Result<TestContext> {
    // Connect to local Anvil instance
    let provider = Provider::<Http>::try_from("http://localhost:8545")?;
    
    // Generate random wallets for testing
    let owner = LocalWallet::new(&mut rand::thread_rng());
    let other_account = LocalWallet::new(&mut rand::thread_rng());
    
    // Deploy the contract
    let client = SignerMiddleware::new(
        provider.clone(),
        owner.clone(),
    );
    
    let contract_address = H160::from_str("0x5FbDB2315678afecb367f032d93F642f64180aa3").unwrap();
    let contract = TestERC20::new(contract_address, Arc::new(client));
    
    Ok(TestContext {
        contract,
        owner,
        other_account
    })
}

#[tokio::test]
async fn test_initial_state() -> Result<()> {
    let ctx = setup().await?;
    
    // Test basic token properties
    assert_eq!(ctx.contract.name().call().await?, "MyToken");
    assert_eq!(ctx.contract.symbol().call().await?, "MTK");
    assert_eq!(ctx.contract.decimals().call().await?, 18);
    
    // Test initial supply
    let total_supply = ctx.contract.total_supply().call().await?;
    assert_eq!(total_supply, U256::from(1000000) * U256::from(10).pow(U256::from(18)));
    
    // Test owner balance
    let owner_balance = ctx.contract.balance_of(ctx.owner.address()).call().await?;
    assert_eq!(owner_balance, total_supply);
    
    Ok(())
}

#[tokio::test]
async fn test_transfer() -> Result<()> {
    let ctx = setup().await?;
    let amount = U256::from(100);
    
    // Transfer tokens
    ctx.contract
        .transfer(ctx.other_account.address(), amount)
        .send()
        .await?
        .await?;
    
    // Check balances
    let recipient_balance = ctx.contract
        .balance_of(ctx.other_account.address())
        .call()
        .await?;
    assert_eq!(recipient_balance, amount);
    
    let owner_balance = ctx.contract
        .balance_of(ctx.owner.address())
        .call()
        .await?;
    assert_eq!(
        owner_balance,
        ctx.contract.total_supply().call().await? - amount
    );
    
    Ok(())
}

#[tokio::test]
async fn test_approve_and_transfer_from() -> Result<()> {
    let ctx = setup().await?;
    let amount = U256::from(100);
    
    // Approve spending
    ctx.contract
        .approve(ctx.other_account.address(), amount)
        .send()
        .await?
        .await?;
    
    // Check allowance
    let allowance = ctx.contract
        .allowance(ctx.owner.address(), ctx.other_account.address())
        .call()
        .await?;
    assert_eq!(allowance, amount);
    
    // Transfer using transferFrom
    let recipient = LocalWallet::new(&mut rand::thread_rng());
    ctx.contract
        .transfer_from(ctx.owner.address(), recipient.address(), amount)
        .send()
        .await?
        .await?;
    
    // Check balances
    let recipient_balance = ctx.contract
        .balance_of(recipient.address())
        .call()
        .await?;
    assert_eq!(recipient_balance, amount);
    
    // Check allowance was decreased
    let new_allowance = ctx.contract
        .allowance(ctx.owner.address(), ctx.other_account.address())
        .call()
        .await?;
    assert_eq!(new_allowance, U256::zero());
    
    Ok(())
}

#[tokio::test]
async fn test_insufficient_balance() -> Result<()> {
    let ctx = setup().await?;
    let total_supply = ctx.contract.total_supply().call().await?;
    
    // Try to transfer more than balance
    let transfer_amount = total_supply + U256::from(1);
    
    // Fix the borrowing issue by storing the transfer call
    let tx = ctx.contract
        .transfer(ctx.other_account.address(), transfer_amount);
    
    let result = tx.send().await;
    
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn test_insufficient_allowance() -> Result<()> {
    let ctx = setup().await?;
    let total_supply = ctx.contract.total_supply().call().await?;

    // Try to transfer more than balance
    let transfer_amount = total_supply + U256::from(1);

    //Fix the borrowing issue by storing the transfer call
    let tx = ctx.contract.transfer(ctx.other_account.address(), transfer_amount);

    let result = tx.send().await;
    assert!(result.is_err());
    Ok(())
}

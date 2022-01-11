//! StarkNet node JSON-RPC related modules.
pub mod api;
pub mod types;

use crate::rpc::{
    api::RpcApi,
    types::{relaxed::H256, request::Call, BlockHashOrTag, BlockNumberOrTag},
};
use jsonrpsee::{
    http_server::{HttpServerBuilder, HttpServerHandle, RpcModule},
    types::Error,
};
use std::{net::SocketAddr, result::Result};

/// Starts the HTTP-RPC server.
pub fn run_server(addr: SocketAddr) -> Result<(HttpServerHandle, SocketAddr), Error> {
    let server = HttpServerBuilder::default().build(addr)?;
    let local_addr = server.local_addr()?;
    let api = RpcApi::default();
    let mut module = RpcModule::new(api);
    module.register_async_method("starknet_getBlockByHash", |params, context| async move {
        let mut params = params.sequence();
        let block_hash = params.next()?;
        let scope = params.optional_next()?;
        context.get_block_by_hash(block_hash, scope).await
    })?;
    module.register_async_method("starknet_getBlockByNumber", |params, context| async move {
        let mut params = params.sequence();
        let block_number = params.next()?;
        let scope = params.optional_next()?;
        context.get_block_by_number(block_number, scope).await
    })?;
    module.register_async_method(
        "starknet_getStateUpdateByHash",
        |params, context| async move {
            let block_hash = params.one::<BlockHashOrTag>()?;
            context.get_state_update_by_hash(block_hash).await
        },
    )?;
    module.register_async_method("starknet_getStorageAt", |params, context| async move {
        let (contract_address, key, block_hash) = params.parse::<(H256, H256, BlockHashOrTag)>()?;
        context
            .get_storage_at(contract_address, key, block_hash)
            .await
    })?;
    module.register_async_method(
        "starknet_getTransactionByHash",
        |params, context| async move {
            let transaction_hash = params.one::<H256>()?;
            context.get_transaction_by_hash(transaction_hash).await
        },
    )?;
    module.register_async_method(
        "starknet_getTransactionByBlockHashAndIndex",
        |params, context| async move {
            let (block_hash, index) = params.parse::<(BlockHashOrTag, u64)>()?;
            context
                .get_transaction_by_block_hash_and_index(block_hash, index)
                .await
        },
    )?;
    module.register_async_method(
        "starknet_getTransactionByBlockNumberAndIndex",
        |params, context| async move {
            let (block_number, index) = params.parse::<(BlockNumberOrTag, u64)>()?;
            context
                .get_transaction_by_block_number_and_index(block_number, index)
                .await
        },
    )?;
    module.register_async_method(
        "starknet_getTransactionReceipt",
        |params, context| async move {
            let transaction_hash = params.one::<H256>()?;
            context.get_transaction_receipt(transaction_hash).await
        },
    )?;
    module.register_async_method("starknet_getCode", |params, context| async move {
        let contract_address = params.one::<H256>()?;
        context.get_code(contract_address).await
    })?;
    module.register_async_method(
        "starknet_getBlockTransactionCountByHash",
        |params, context| async move {
            let block_hash = params.one::<BlockHashOrTag>()?;
            context
                .get_block_transaction_count_by_hash(block_hash)
                .await
        },
    )?;
    module.register_async_method(
        "starknet_getBlockTransactionCountByNumber",
        |params, context| async move {
            let block_number = params.one::<BlockNumberOrTag>()?;
            context
                .get_block_transaction_count_by_number(block_number)
                .await
        },
    )?;
    module.register_async_method("starknet_call", |params, context| async move {
        let mut params = params.sequence();
        let request = params.next::<Call>()?;
        let block_hash = params.next::<BlockHashOrTag>()?;
        context.call(request, block_hash).await
    })?;
    module.register_async_method("starknet_blockNumber", |_, context| async move {
        context.block_number().await
    })?;
    module.register_async_method("starknet_chainId", |_, context| async move {
        context.chain_id().await
    })?;
    module.register_async_method("starknet_pendingTransactions", |_, context| async move {
        context.pending_transactions().await
    })?;
    module.register_async_method("starknet_protocolVersion", |_, context| async move {
        context.protocol_version().await
    })?;
    module.register_async_method("starknet_syncing", |_, context| async move {
        context.chain_id().await
    })?;
    server.start(module).map(|handle| (handle, local_addr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::{run_server, types::relaxed};
    use assert_matches::assert_matches;
    use jsonrpsee::{
        http_client::{HttpClient, HttpClientBuilder},
        rpc_params,
        types::traits::Client,
    };
    use std::{
        net::{Ipv4Addr, SocketAddrV4},
        str::FromStr,
    };
    use web3::types::H256;

    /// Helper rpc client
    fn client(addr: SocketAddr) -> HttpClient {
        HttpClientBuilder::default()
            .build(format!("http://{}", addr))
            .expect("Failed to create HTTP-RPC client")
    }

    lazy_static::lazy_static! {
        static ref GENESIS_BLOCK_HASH: H256 = H256::from_str("0x07d328a71faf48c5c3857e99f20a77b18522480956d1cd5bff1ff2df3c8b427b").unwrap();
        static ref INVALID_BLOCK_HASH: H256 = H256::from_str("0x13d8d8bb5716cd3f16e54e3a6ff1a50542461d9022e5f4dec7a4b064041ab8d7").unwrap();
        static ref UNKNOWN_BLOCK_HASH: H256 = H256::from_str("0x03c85a69453e63fd475424ecc70438bd855cd76e6f0d5dec0d0dd56e0f7a771c").unwrap();
        static ref PRE_DEPLOY_CONTRACT_BLOCK_HASH: H256 = H256::from_str("0x05ef884a311df4339c8df791ce19bf305d7cf299416666b167bc56dd2d1f435f").unwrap();
        static ref DEPLOY_CONTRACT_BLOCK_HASH: H256 = H256::from_str("0x07177acba67cb659e336abb3a158c8d29770b87b1b62e2bfa94cd376b72d34c5").unwrap();
        static ref INVOKE_CONTRACT_BLOCK_HASH: H256 = H256::from_str("0x03871c8a0c3555687515a07f365f6f5b1d8c2ae953f7844575b8bde2b2efed27").unwrap();
        static ref VALID_TX_HASH: relaxed::H256 = H256::from_str("0x0493d8fab73af67e972788e603aee18130facd3c7685f16084ecd98b07153e24").unwrap().into();
        static ref INVALID_TX_HASH: relaxed::H256 = H256::from_str("0x1493d8fab73af67e972788e603aee18130facd3c7685f16084ecd98b07153e24").unwrap().into();
        static ref UNKNOWN_TX_HASH: relaxed::H256 = H256::from_str("0x015e4bb72df94be3044139fea2116c4d54add05cf9ef8f35aea114b5cea94713").unwrap().into();
        static ref VALID_CONTRACT_ADDR: relaxed::H256 = H256::from_str("0x06fbd460228d843b7fbef670ff15607bf72e19fa94de21e29811ada167b4ca39").unwrap().into();
        static ref INVALID_CONTRACT_ADDR: relaxed::H256 = H256::from_str("0x16fbd460228d843b7fbef670ff15607bf72e19fa94de21e29811ada167b4ca39").unwrap().into();
        static ref UNKNOWN_CONTRACT_ADDR: relaxed::H256 = H256::from_str("0x0739636829ad5205d81af792a922a40e35c0ec7a72f4859843ee2e2a0d6f0af0").unwrap().into();
        static ref VALID_ENTRY_POINT: relaxed::H256 = H256::from_str("0x0362398bec32bc0ebb411203221a35a0301193a96f317ebe5e40be9f60d15320").unwrap().into();
        static ref LOCALHOST: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
    }

    mod error {
        lazy_static::lazy_static! {
            pub static ref CONTRACT_NOT_FOUND: (i64, String) = (-32020, "Contract not found".to_owned());
            pub static ref INVALID_SELECTOR: (i64, String) = (-32021, "Invalid message selector".to_owned());
            pub static ref INVALID_CALL_DATA: (i64, String) = (-32022, "Invalid call data".to_owned());
            pub static ref INVALID_KEY: (i64, String) = (-32023, "Invalid storage key".to_owned());
            pub static ref INVALID_BLOCK_HASH: (i64, String) = (-32024, "Invalid block hash".to_owned());
            pub static ref INVALID_TX_HASH: (i64, String) = (-32025, "Invalid transaction hash".to_owned());
            pub static ref INVALID_BLOCK_NUMBER: (i64, String) = (-32025, "Invalid block number".to_owned());
        }
    }

    fn get_err(json_str: String) -> (i64, String) {
        let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        (
            v["error"]["code"].as_i64().unwrap(),
            v["error"]["message"].as_str().unwrap().to_owned(),
        )
    }

    mod get_block_by_hash {
        use super::*;
        use crate::rpc::types::{reply::Block, BlockHashOrTag, Tag};

        #[tokio::test]
        #[ignore = "currently causes HTTP 504"]
        async fn genesis() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Hash(*GENESIS_BLOCK_HASH));
            client(addr)
                .request::<Block>("starknet_getBlockByHash", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn latest() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Tag(Tag::Latest));
            client(addr)
                .request::<Block>("starknet_getBlockByHash", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn pending() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Tag(Tag::Pending));
            client(addr)
                .request::<Block>("starknet_getBlockByHash", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn not_found() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Hash(*UNKNOWN_BLOCK_HASH));
            let error = client(addr)
                .request::<Block>("starknet_getBlockByHash", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_BLOCK_HASH)
            );
        }

        #[tokio::test]
        async fn invalid_block_hash() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Hash(*INVALID_BLOCK_HASH));
            let error = client(addr)
                .request::<Block>("starknet_getBlockByHash", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_BLOCK_HASH)
            );
        }
    }

    mod get_block_by_number {
        use super::*;
        use crate::rpc::types::{reply::Block, BlockNumberOrTag, Tag};

        #[tokio::test]
        async fn genesis() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockNumberOrTag::Number(0));
            client(addr)
                .request::<Block>("starknet_getBlockByNumber", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn latest() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockNumberOrTag::Tag(Tag::Latest));
            client(addr)
                .request::<Block>("starknet_getBlockByNumber", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn pending() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockNumberOrTag::Tag(Tag::Pending));
            client(addr)
                .request::<Block>("starknet_getBlockByNumber", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn invalid_number() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockNumberOrTag::Number(u64::MAX));
            let error = client(addr)
                .request::<Block>("starknet_getBlockByNumber", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_BLOCK_NUMBER)
            );
        }
    }

    mod get_state_update_by_hash {
        use super::*;
        use crate::rpc::types::{BlockHashOrTag, Tag};

        #[tokio::test]
        #[should_panic]
        async fn genesis() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Hash(*GENESIS_BLOCK_HASH));
            client(addr)
                .request::<()>("starknet_getStateUpdateByHash", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        #[should_panic]
        async fn latest() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Tag(Tag::Latest));
            client(addr)
                .request::<()>("starknet_getStateUpdateByHash", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        #[should_panic]
        async fn pending() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Tag(Tag::Pending));
            client(addr)
                .request::<()>("starknet_getStateUpdateByHash", params)
                .await
                .unwrap();
        }
    }

    mod get_storage_at {
        use super::*;
        use crate::rpc::types::{relaxed, BlockHashOrTag, Tag};
        use web3::types::H256;

        lazy_static::lazy_static! {
            static ref VALID_KEY: relaxed::H256 = H256::from_str("0x0206F38F7E4F15E87567361213C28F235CCCDAA1D7FD34C9DB1DFE9489C6A091").unwrap().into();
            static ref INVALID_KEY: relaxed::H256 = H256::from_str("0x1206F38F7E4F15E87567361213C28F235CCCDAA1D7FD34C9DB1DFE9489C6A091").unwrap().into();
        }

        #[tokio::test]
        async fn invalid_contract() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                *INVALID_CONTRACT_ADDR,
                *VALID_KEY,
                BlockHashOrTag::Tag(Tag::Latest)
            );
            let error = client(addr)
                .request::<relaxed::H256>("starknet_getStorageAt", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::CONTRACT_NOT_FOUND)
            );
        }

        #[tokio::test]
        async fn invalid_key() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                *VALID_CONTRACT_ADDR,
                *INVALID_KEY,
                BlockHashOrTag::Tag(Tag::Latest)
            );
            let error = client(addr)
                .request::<relaxed::H256>("starknet_getStorageAt", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_KEY)
            );
        }

        #[tokio::test]
        async fn invalid_key_is_zero() {
            // Invalid key of value zero results with storage value of zero
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                *VALID_CONTRACT_ADDR,
                relaxed::H256::from(H256::zero()),
                BlockHashOrTag::Tag(Tag::Latest)
            );
            client(addr)
                .request::<relaxed::H256>("starknet_getStorageAt", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn block_not_found() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                *VALID_CONTRACT_ADDR,
                *VALID_KEY,
                BlockHashOrTag::Hash(*UNKNOWN_BLOCK_HASH)
            );
            let error = client(addr)
                .request::<relaxed::H256>("starknet_getStorageAt", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_BLOCK_HASH)
            );
        }

        #[tokio::test]
        async fn invalid_block_hash() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                *VALID_CONTRACT_ADDR,
                *VALID_KEY,
                BlockHashOrTag::Hash(*INVALID_BLOCK_HASH)
            );
            let error = client(addr)
                .request::<relaxed::H256>("starknet_getStorageAt", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_BLOCK_HASH)
            );
        }

        #[tokio::test]
        async fn latest_invoke_block() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                *VALID_CONTRACT_ADDR,
                *VALID_KEY,
                BlockHashOrTag::Hash(*INVOKE_CONTRACT_BLOCK_HASH)
            );
            client(addr)
                .request::<relaxed::H256>("starknet_getStorageAt", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn latest_block() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                *VALID_CONTRACT_ADDR,
                *VALID_KEY,
                BlockHashOrTag::Tag(Tag::Latest)
            );
            client(addr)
                .request::<relaxed::H256>("starknet_getStorageAt", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn pending_block() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                *VALID_CONTRACT_ADDR,
                *VALID_KEY,
                BlockHashOrTag::Tag(Tag::Pending)
            );
            client(addr)
                .request::<relaxed::H256>("starknet_getStorageAt", params)
                .await
                .unwrap();
        }
    }

    mod get_transaction_by_hash {
        use super::*;
        use crate::rpc::types::reply::Transaction;

        #[tokio::test]
        async fn accepted() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(*VALID_TX_HASH);
            client(addr)
                .request::<Transaction>("starknet_getTransactionByHash", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn invalid_hash() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(*INVALID_TX_HASH);
            let error = client(addr)
                .request::<Transaction>("starknet_getTransactionByHash", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_TX_HASH)
            );
        }

        #[tokio::test]
        async fn unknown_hash() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(*UNKNOWN_TX_HASH);
            let error = client(addr)
                .request::<Transaction>("starknet_getTransactionByHash", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_TX_HASH)
            );
        }
    }

    mod get_transaction_by_block_hash_and_index {
        use super::*;
        use crate::rpc::types::{reply::Transaction, BlockHashOrTag, Tag};

        #[tokio::test]
        #[ignore = "currently causes HTTP 504"]
        async fn genesis() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Hash(*GENESIS_BLOCK_HASH), 0u64);
            client(addr)
                .request::<Transaction>("starknet_getTransactionByBlockHashAndIndex", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn latest() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Tag(Tag::Latest), 0u64);
            client(addr)
                .request::<Transaction>("starknet_getTransactionByBlockHashAndIndex", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn pending() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Tag(Tag::Pending), 0u64);
            client(addr)
                .request::<Transaction>("starknet_getTransactionByBlockHashAndIndex", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn invalid_block() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Hash(*INVALID_BLOCK_HASH), 0u64);
            let error = client(addr)
                .request::<Transaction>("starknet_getTransactionByBlockHashAndIndex", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_BLOCK_HASH)
            );
        }

        #[tokio::test]
        async fn unknown_block() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Hash(*UNKNOWN_BLOCK_HASH), 0u64);
            let error = client(addr)
                .request::<Transaction>("starknet_getTransactionByBlockHashAndIndex", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_BLOCK_HASH)
            );
        }

        #[tokio::test]
        async fn invalid_transaction_index() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Hash(*DEPLOY_CONTRACT_BLOCK_HASH), u64::MAX);
            client(addr)
                .request::<Transaction>("starknet_getTransactionByBlockHashAndIndex", params)
                .await
                .unwrap_err();
        }
    }

    mod get_transaction_by_block_number_and_index {
        use super::*;
        use crate::rpc::types::{reply::Transaction, BlockNumberOrTag, Tag};

        #[tokio::test]
        async fn genesis() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockNumberOrTag::Number(0), 0);
            client(addr)
                .request::<Transaction>("starknet_getTransactionByBlockNumberAndIndex", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn latest() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockNumberOrTag::Tag(Tag::Latest), 0);
            client(addr)
                .request::<Transaction>("starknet_getTransactionByBlockNumberAndIndex", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn pending() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockNumberOrTag::Tag(Tag::Pending), 0);
            client(addr)
                .request::<Transaction>("starknet_getTransactionByBlockNumberAndIndex", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn invalid_block() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockNumberOrTag::Number(u64::MAX), 0);
            let error = client(addr)
                .request::<Transaction>("starknet_getTransactionByBlockNumberAndIndex", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_BLOCK_NUMBER)
            );
        }

        #[tokio::test]
        async fn invalid_transaction_index() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockNumberOrTag::Tag(Tag::Latest), u64::MAX);
            client(addr)
                .request::<Transaction>("starknet_getTransactionByBlockNumberAndIndex", params)
                .await
                .unwrap_err();
        }
    }

    mod get_transaction_receipt {
        use super::*;
        use crate::rpc::types::reply::TransactionReceipt;

        #[tokio::test]
        async fn accepted() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(*VALID_TX_HASH);
            client(addr)
                .request::<TransactionReceipt>("starknet_getTransactionReceipt", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn invalid() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(*INVALID_TX_HASH);
            let error = client(addr)
                .request::<TransactionReceipt>("starknet_getTransactionReceipt", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_TX_HASH)
            );
        }

        #[tokio::test]
        async fn unknown() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(*UNKNOWN_TX_HASH);
            let error = client(addr)
                .request::<TransactionReceipt>("starknet_getTransactionReceipt", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_TX_HASH)
            );
        }
    }

    mod get_code {
        use super::*;
        use crate::sequencer::reply::Code;

        #[tokio::test]
        async fn invalid_contract_address() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(*INVALID_CONTRACT_ADDR);
            let error = client(addr)
                .request::<Code>("starknet_getCode", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::CONTRACT_NOT_FOUND)
            );
        }

        #[tokio::test]
        async fn unknown_contract_address() {
            // At the moment a valid address from mainnet causes an empty but valid reply on goerli
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(*UNKNOWN_CONTRACT_ADDR);
            client(addr)
                .request::<Code>("starknet_getCode", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn success() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(*VALID_CONTRACT_ADDR);
            client(addr)
                .request::<Code>("starknet_getCode", params)
                .await
                .unwrap();
        }
    }

    mod get_block_transaction_count_by_hash {
        use super::*;
        use crate::rpc::types::{BlockHashOrTag, Tag};

        #[tokio::test]
        #[ignore = "currently causes HTTP 504"]
        async fn genesis() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Hash(*GENESIS_BLOCK_HASH));
            client(addr)
                .request::<u64>("starknet_getBlockTransactionCountByHash", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn latest() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Tag(Tag::Latest));
            client(addr)
                .request::<u64>("starknet_getBlockTransactionCountByHash", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn pending() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Tag(Tag::Pending));
            client(addr)
                .request::<u64>("starknet_getBlockTransactionCountByHash", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn invalid() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Hash(*INVALID_BLOCK_HASH));
            let error = client(addr)
                .request::<u64>("starknet_getBlockTransactionCountByHash", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_BLOCK_HASH)
            );
        }

        #[tokio::test]
        async fn unknown() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockHashOrTag::Hash(*UNKNOWN_BLOCK_HASH));
            let error = client(addr)
                .request::<u64>("starknet_getBlockTransactionCountByHash", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_BLOCK_HASH)
            );
        }
    }

    mod get_block_transaction_count_by_number {
        use super::*;
        use crate::rpc::types::{BlockNumberOrTag, Tag};

        #[tokio::test]
        async fn genesis() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockNumberOrTag::Number(0));
            client(addr)
                .request::<u64>("starknet_getBlockTransactionCountByNumber", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn latest() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockNumberOrTag::Tag(Tag::Latest));
            client(addr)
                .request::<u64>("starknet_getBlockTransactionCountByNumber", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn pending() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockNumberOrTag::Tag(Tag::Pending));
            client(addr)
                .request::<u64>("starknet_getBlockTransactionCountByNumber", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn invalid() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(BlockNumberOrTag::Number(u64::MAX));
            let error = client(addr)
                .request::<u64>("starknet_getBlockTransactionCountByNumber", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_BLOCK_NUMBER)
            );
        }
    }

    mod call {
        use super::*;
        use crate::rpc::types::{request::Call, BlockHashOrTag, Tag};
        use web3::types::H256;

        lazy_static::lazy_static! {
            static ref CALL_DATA: Vec<H256> = vec![H256::from_str("0x0000000000000000000000000000000000000000000000000000000000001234").unwrap()];
        }

        #[tokio::test]
        async fn latest_invoked_block() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                Call {
                    calldata: CALL_DATA.clone(),
                    contract_address: **VALID_CONTRACT_ADDR,
                    entry_point_selector: **VALID_ENTRY_POINT,
                },
                BlockHashOrTag::Hash(*INVOKE_CONTRACT_BLOCK_HASH)
            );
            client(addr)
                .request::<Vec<relaxed::H256>>("starknet_call", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn latest_block() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                Call {
                    calldata: CALL_DATA.clone(),
                    contract_address: **VALID_CONTRACT_ADDR,
                    entry_point_selector: **VALID_ENTRY_POINT,
                },
                BlockHashOrTag::Tag(Tag::Latest)
            );
            client(addr)
                .request::<Vec<relaxed::H256>>("starknet_call", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn pending_block() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                Call {
                    calldata: CALL_DATA.clone(),
                    contract_address: **VALID_CONTRACT_ADDR,
                    entry_point_selector: **VALID_ENTRY_POINT,
                },
                BlockHashOrTag::Tag(Tag::Pending)
            );
            client(addr)
                .request::<Vec<relaxed::H256>>("starknet_call", params)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn invalid_entry_point() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                Call {
                    calldata: CALL_DATA.clone(),
                    contract_address: **VALID_CONTRACT_ADDR,
                    entry_point_selector: H256::zero(),
                },
                BlockHashOrTag::Tag(Tag::Latest)
            );
            let error = client(addr)
                .request::<Vec<relaxed::H256>>("starknet_call", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_SELECTOR)
            );
        }

        #[tokio::test]
        async fn invalid_contract_address() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                Call {
                    calldata: CALL_DATA.clone(),
                    contract_address: **INVALID_CONTRACT_ADDR,
                    entry_point_selector: **VALID_ENTRY_POINT,
                },
                BlockHashOrTag::Tag(Tag::Latest)
            );
            let error = client(addr)
                .request::<Vec<relaxed::H256>>("starknet_call", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::CONTRACT_NOT_FOUND)
            );
        }

        #[tokio::test]
        async fn invalid_call_data() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                Call {
                    calldata: vec![],
                    contract_address: **VALID_CONTRACT_ADDR,
                    entry_point_selector: **VALID_ENTRY_POINT,
                },
                BlockHashOrTag::Tag(Tag::Latest)
            );
            let error = client(addr)
                .request::<Vec<relaxed::H256>>("starknet_call", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_CALL_DATA)
            );
        }

        #[tokio::test]
        async fn uninitialized_contract() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                Call {
                    calldata: CALL_DATA.clone(),
                    contract_address: **VALID_CONTRACT_ADDR,
                    entry_point_selector: **VALID_ENTRY_POINT,
                },
                BlockHashOrTag::Hash(*PRE_DEPLOY_CONTRACT_BLOCK_HASH)
            );
            let error = client(addr)
                .request::<Vec<relaxed::H256>>("starknet_call", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::CONTRACT_NOT_FOUND)
            );
        }

        #[tokio::test]
        async fn invalid_block_hash() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                Call {
                    calldata: CALL_DATA.clone(),
                    contract_address: **VALID_CONTRACT_ADDR,
                    entry_point_selector: **VALID_ENTRY_POINT,
                },
                BlockHashOrTag::Hash(*INVALID_BLOCK_HASH)
            );
            let error = client(addr)
                .request::<Vec<relaxed::H256>>("starknet_call", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_BLOCK_HASH)
            );
        }

        #[tokio::test]
        async fn unknown_block_hash() {
            let (_handle, addr) = run_server(*LOCALHOST).unwrap();
            let params = rpc_params!(
                Call {
                    calldata: CALL_DATA.clone(),
                    contract_address: **VALID_CONTRACT_ADDR,
                    entry_point_selector: **VALID_ENTRY_POINT,
                },
                BlockHashOrTag::Hash(*UNKNOWN_BLOCK_HASH)
            );
            let error = client(addr)
                .request::<Vec<relaxed::H256>>("starknet_call", params)
                .await
                .unwrap_err();
            assert_matches!(
                error,
                Error::Request(s) => assert_eq!(get_err(s), *error::INVALID_BLOCK_HASH)
            );
        }
    }

    #[tokio::test]
    async fn block_number() {
        let (_handle, addr) = run_server(*LOCALHOST).unwrap();
        let params = rpc_params!();
        client(addr)
            .request::<u64>("starknet_blockNumber", params)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[should_panic]
    async fn chain_id() {
        let (_handle, addr) = run_server(*LOCALHOST).unwrap();
        let params = rpc_params!();
        client(addr)
            .request::<relaxed::H256>("starknet_chainId", params)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[should_panic]
    async fn pending_transactions() {
        let (_handle, addr) = run_server(*LOCALHOST).unwrap();
        let params = rpc_params!();
        client(addr)
            .request::<()>("starknet_pendingTransactions", params)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[should_panic]
    async fn protocol_version() {
        let (_handle, addr) = run_server(*LOCALHOST).unwrap();
        let params = rpc_params!();
        client(addr)
            .request::<relaxed::H256>("starknet_protocolVersion", params)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[should_panic]
    async fn syncing() {
        let (_handle, addr) = run_server(*LOCALHOST).unwrap();
        let params = rpc_params!();
        use crate::rpc::types::reply::Syncing;
        client(addr)
            .request::<Syncing>("starknet_syncing", params)
            .await
            .unwrap();
    }
}
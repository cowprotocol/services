use utoipa::{
    openapi::{InfoBuilder, OpenApiBuilder, ServerBuilder},
    OpenApi,
};

fn main() {
    let servers = vec![
        ServerBuilder::new()
            .description(Some("Mainnet (Prod)"))
            .url("https://api.cow.fi/mainnet")
            .build(),
        ServerBuilder::new()
            .description(Some("Mainnet (Staging)"))
            .url("https://barn.api.cow.fi/mainnet")
            .build(),
        ServerBuilder::new()
            .description(Some("Görli (Prod)"))
            .url("https://api.cow.fi/goerli")
            .build(),
        ServerBuilder::new()
            .description(Some("Görli (Staging)"))
            .url("https://barn.api.cow.fi/goerli")
            .build(),
        ServerBuilder::new()
            .description(Some("Sepolia (Prod)"))
            .url("https://api.cow.fi/sepolia")
            .build(),
        ServerBuilder::new()
            .description(Some("Sepolia (Staging)"))
            .url("https://barn.api.cow.fi/sepolia")
            .build(),
        ServerBuilder::new()
            .description(Some("Gnosis Chain (Prod)"))
            .url("https://api.cow.fi/xdai")
            .build(),
        ServerBuilder::new()
            .description(Some("Gnosis Chain (Staging)"))
            .url("https://barn.api.cow.fi/xdai")
            .build(),
        ServerBuilder::new()
            .description(Some("Local"))
            .url("http://localhost:8080")
            .build(),
    ];

    let info = InfoBuilder::new()
        .title("Orderbook API")
        .contact(None)
        .version("0.0.1");

    let builder: OpenApiBuilder = orderbook::api::ApiDoc::openapi().into();
    let doc = builder.servers(Some(servers)).info(info).build();

    println!("{}", doc.to_yaml().unwrap());
}

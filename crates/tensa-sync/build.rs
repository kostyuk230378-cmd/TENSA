//! Сборка gRPC-кода из proto Контракта 8 (фича `grpc`).
//!
//! Единственный источник истины — `teh_doc/contracts/proto/` (не копируем
//! в крейт, чтобы избежать дублирования контрактов).
//! protoc поставляется крейтом protoc-bin-vendored (системный protoc не нужен).

fn main() {
    if std::env::var("CARGO_FEATURE_GRPC").is_err() {
        return;
    }

    // Vendored protoc: путь сообщаем prost-build через переменную PROTOC.
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
    std::env::set_var("PROTOC", protoc);

    let manifest =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let proto_dir = manifest.join("../../teh_doc/contracts/proto");

    let files = [
        "common.proto",
        "sync.proto",
        "node.proto",
        "data.proto",
        "voice.proto",
        "query.proto",
    ];
    let protos: Vec<_> = files.iter().map(|f| proto_dir.join(f)).collect();
    for p in &protos {
        println!("cargo:rerun-if-changed={}", p.display());
    }

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&protos, &[proto_dir])
        .expect("compile protos (Контракт 8)");
}

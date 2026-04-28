{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
    nativeBuildInputs = with pkgs; [ 
        #Rust
        rustc
        cargo
        gcc
        rustfmt
        clippy
        # Dependencies
        gtk4
        pkg-config
        libadwaita
    ];

    RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
}

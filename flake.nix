{
  description = "The Herta — голосовой ассистент и TUI на Rust (Honkai: Star Rail)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        herta = pkgs.rustPlatform.buildRustPackage {
          pname = "herta-voiceassistant";
          version = "0.4.0";
          src = self;
          cargoLock.lockFile = ./Cargo.lock;
          # rustls/ring — нативный OpenSSL не нужен.
          nativeBuildInputs = [ pkgs.pkg-config ];
          # Собираем только бинарь CLI.
          cargoBuildFlags = [ "--bin" "herta" ];
          doCheck = true;
          meta = with pkgs.lib; {
            description = "Великая Герта — голосовой ассистент и TUI на Rust";
            homepage = "https://github.com/vadim-khristenko/the-herta-voiceassistant-pwd-by-rust";
            license = licenses.mit;
            mainProgram = "herta";
          };
        };
      in
      {
        packages.default = herta;
        packages.herta = herta;

        apps.default = flake-utils.lib.mkApp { drv = herta; name = "herta"; };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ herta ];
          packages = with pkgs; [ rustc cargo clippy rustfmt rust-analyzer ];
        };
      });
}

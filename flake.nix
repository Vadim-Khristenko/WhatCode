{
  description = "WhatCode — ассистент для разработки и TUI на Rust с выбираемыми персонами";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        whatcode = pkgs.rustPlatform.buildRustPackage {
          pname = "whatcode";
          version = "0.6.0";
          src = self;
          cargoLock.lockFile = ./Cargo.lock;
          # rustls/ring — нативный OpenSSL не нужен.
          nativeBuildInputs = [ pkgs.pkg-config ];
          # Собираем только бинарь CLI.
          cargoBuildFlags = [ "--bin" "whatcode" ];
          doCheck = true;
          meta = with pkgs.lib; {
            description = "WhatCode — ассистент для разработки и TUI на Rust с выбираемыми персонами";
            homepage = "https://github.com/vadim-khristenko/WhatCode";
            license = licenses.mit;
            mainProgram = "whatcode";
          };
        };
      in
      {
        packages.default = whatcode;
        packages.whatcode = whatcode;

        apps.default = flake-utils.lib.mkApp { drv = whatcode; name = "whatcode"; };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ whatcode ];
          packages = with pkgs; [ rustc cargo clippy rustfmt rust-analyzer ];
        };
      });
}

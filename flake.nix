{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, crane }:

    flake-utils.lib.eachSystem [
      flake-utils.lib.system.x86_64-linux
      flake-utils.lib.system.x86_64-darwin
    ]
      (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          craneLib = crane.lib.${system};
        in
        rec {
          packages.taskdep =
            craneLib.buildPackage {
              src = craneLib.cleanCargoSource ./.;
              cargoTestCommand = "";
              buildInputs = with pkgs; [
                graphviz
                libiconv
                darwin.apple_sdk.frameworks.Security
              ];
            };
          packages.default = packages.taskdep;
          devShells.default = pkgs.mkShell {
            buildInputs = [ pkgs.cargo pkgs.rustc pkgs.go-task ];
          };
        });

}

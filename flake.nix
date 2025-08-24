{
  description = "Draw dependency graphs from a Taskfile";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    shell-utils.url = "github:waltermoreira/shell-utils";
  };

  outputs = { self, nixpkgs, flake-utils, crane, shell-utils }:

    with flake-utils.lib; eachSystem [
      system.x86_64-linux
      system.x86_64-darwin
      system.aarch64-darwin
    ]
      (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          craneLib = crane.lib.${system};
          shell = shell-utils.myShell.${system};
        in
        rec {
          packages.taskdep =
            craneLib.buildPackage {
              src = craneLib.cleanCargoSource ./.;
              buildInputs = with pkgs; [
                graphviz
                libiconv
              ];
              DOTPATH = "${pkgs.graphviz}/bin/dot";
            };
          packages.default = packages.taskdep;
          devShells.default = shell {
            packages = with pkgs; [
              packages.taskdep
              cargo
              rustc
              go-task
            ];
          };
        });

}

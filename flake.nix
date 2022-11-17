{
  description = "Draw dependency graphs from a Taskfile";

  # Modify taskdep to get a path for `dot` that we pass at compilation time
  # using a patch phase? or configure phase?
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    shell-utils.url = "github:waltermoreira/shell-utils";
  };

  outputs = { self, nixpkgs, flake-utils, crane, shell-utils }:

    flake-utils.lib.eachSystem [
      flake-utils.lib.system.x86_64-linux
      flake-utils.lib.system.x86_64-darwin
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
              configurePhase = ''
                echo "In configure"
              '';
              src = craneLib.cleanCargoSource ./.;
              #cargoTestCommand = "";
              buildInputs = with pkgs; [
                graphviz
                libiconv
              ];
            };
          packages.default = packages.taskdep;
          devShells.default = shell {
            packages = [
              pkgs.graphviz
              packages.taskdep
              pkgs.cargo
              pkgs.rustc
              pkgs.go-task
            ];
          };
        });

}

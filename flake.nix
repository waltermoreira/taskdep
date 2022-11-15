{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:

    flake-utils.lib.eachSystem [
      flake-utils.lib.system.x86_64-linux
      flake-utils.lib.system.x86_64-darwin
    ]
      (system:
        let pkgs = nixpkgs.legacyPackages.${system};
        in
        rec {
          packages.taskdep =
            pkgs.stdenv.mkDerivation {
              name = "taskdep";
              src = self;
              buildPhase = ''
                cargo build
              '';
              installPhase = ''
                mkdir -p $out/bin; cp target/debug/taskdep $out/bin
              '';
              buildInputs = [ pkgs.rustc pkgs.cargo ];
            }
          ;
          packages.default = packages.taskdep;
          devShells.default = pkgs.mkShell { 
            buildInputs = [ pkgs.cargo pkgs.rustc pkgs.go-task ]; };
        });

}

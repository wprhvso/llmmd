{
  description = "llmmd";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      version = "0.1.7";

      wheels = {
        x86_64-linux = {
          platform = "manylinux_2_28_x86_64";
          hash = "sha256-auuRqNxCzmcmveuFqKFpUAy0H4nBZSFk3pBC7iSrk+E=";
        };
        aarch64-linux = {
          platform = "manylinux_2_28_aarch64";
          hash = "sha256-lVoeAAlOpkWWP8G3cervaNObyNVn+NQeW57fP9akuZg=";
        };
      };

      systems = builtins.attrNames wheels;

      forAllSystems = nixpkgs.lib.genAttrs systems;

      mkLlmmd =
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          python = pkgs.python314;
          wheel = wheels.${system};
        in
        python.pkgs.buildPythonPackage {
          pname = "llmmd";
          inherit version;
          format = "wheel";

          src = python.pkgs.fetchPypi {
            pname = "llmmd";
            inherit version;
            format = "wheel";
            dist = "cp314";
            python = "cp314";
            abi = "abi3";
            inherit (wheel) platform hash;
          };

          pythonImportsCheck = [ "llmmd" ];

          meta = {
            description = "Markdown to Telegram entities converter";
            mainProgram = "llmmd";
            platforms = systems;
          };
        };
    in
    {
      packages = forAllSystems (
        system:
        let
          llmmd = mkLlmmd system;
        in
        {
          inherit llmmd;
          default = llmmd;
        }
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/llmmd";
        };
      });

      overlays.default = final: prev: {
        pythonPackagesExtensions = (prev.pythonPackagesExtensions or [ ]) ++ [
          (pyFinal: _pyPrev: { llmmd = mkLlmmd final.stdenv.hostPlatform.system; })
        ];
      };

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            packages = [
              (pkgs.python314.withPackages (_ps: [ (mkLlmmd system) ]))
            ];
          };
        }
      );
    };
}

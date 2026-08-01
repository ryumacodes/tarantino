{
  description = "Tarantino Linux development and verification environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          gstPlugins = with pkgs.gst_all_1; [
            gstreamer
            gst-plugins-base
            gst-plugins-good
            gst-plugins-bad
            gst-plugins-ugly
            gst-plugin-pipewire
          ];
          runtimeLibraries = with pkgs; [
            alsa-lib
            ffmpeg
            gtk3
            libayatana-appindicator
            openssl
            pipewire
            webkitgtk_4_1
          ];
        in {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              clang
              curl
              file
              nodejs_22
              pkg-config
              pnpm
              rustc
              rustfmt
              wget
              xdotool
              xdg-desktop-portal
            ] ++ gstPlugins ++ runtimeLibraries;

            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibraries;
            GST_PLUGIN_SYSTEM_PATH_1_0 =
              pkgs.lib.makeSearchPath "lib/gstreamer-1.0" gstPlugins;
            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          };
        });
    };
}

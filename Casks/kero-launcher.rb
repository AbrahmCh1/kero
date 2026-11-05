cask "kero-launcher" do
  version "0.2.1"
  sha256 "983539e75f27017081f6fa03a1800012dc2ba753a614637e73ec2726fb440ab3"

  url "https://github.com/AbrahmCh1/kero/releases/download/v#{version}/kero-#{version}-macos-universal.dmg"
  name "Kero"
  desc "Lanzador estilo Spotlight, ligero y rápido"
  homepage "https://github.com/AbrahmCh1/kero"

  livecheck do
    url :url
    strategy :github_latest
  end

  conflicts_with cask: "kero"
  depends_on macos: ">= :big_sur"

  app "Kero.app"
  binary "#{appdir}/Kero.app/Contents/MacOS/kero"

  # Sin launchctl ni delete: Homebrew también los ejecuta al actualizar, y
  # cerraría Kero sin volver a abrirlo y borraría el inicio automático.
  uninstall quit: "com.acysolutions.kero"

  zap trash: [
    "~/Library/Application Support/Kero",
    "~/Library/LaunchAgents/com.acysolutions.kero.plist",
  ]

  caveats <<~EOS
    Este cask usa el token "kero-launcher" para no chocar con el cask oficial "kero",
    que es otra app y también instala Kero.app. No instales ambos a la vez.
  EOS
end

<p align="center">
  <img src="assets/kero.png" width="96" alt="Kero">
</p>

<h1 align="center">Kero</h1>

<p align="center">
  Lanzador estilo Spotlight: abre apps, encuentra archivos, calcula y convierte,<br>
  sin consumir memoria ni CPU mientras no lo usas.
</p>

<p align="center">
  <a href="https://github.com/AbrahmCh1/kero/actions/workflows/ci.yml"><img src="https://github.com/AbrahmCh1/kero/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/AbrahmCh1/kero/releases/latest"><img src="https://img.shields.io/github/v/release/AbrahmCh1/kero" alt="Release"></a>
  <img src="https://img.shields.io/badge/plataformas-Windows%20%7C%20macOS%20%7C%20Linux-555" alt="Plataformas">
  <a href="LICENSE"><img src="https://img.shields.io/badge/licencia-MIT-555" alt="Licencia MIT"></a>
</p>

---

## Por qué Kero

| | Kero (Windows, medido) |
|---|---|
| Memoria en reposo | **≈12 MB** privados; <1 MB de working set tras 10 s oculto |
| CPU en reposo | 0 % (sólo se despierta con el atajo) |
| Búsqueda | **1–2 ms** sobre 50 000 archivos + 250 apps |
| Indexado | ≈0,5 s para 50 000 archivos; arranque instantáneo desde caché |
| Ejecutable | ≈6 MB, sin runtime ni dependencias |

Escrito en Rust. La interfaz se dibuja con un rasterizador propio en CPU, así que no carga drivers de GPU (que en equipos con gráficos híbridos cuestan ~50 MB por sí solos).

## Qué hace

- **Apps** — todas las del sistema (en Windows también las de Microsoft Store).
- **Archivos y carpetas** — búsqueda difusa en las carpetas que elijas; el índice se actualiza solo al crear, borrar o renombrar.
- **Calculadora y unidades** — `1500 * 1.16`, `sqrt(2)`, `5 km to mi`, `0xff to decimal`. **Enter copia el resultado.**
- **Colores** — `#3b82f6` o `rgb(59,130,246)` → HEX, RGB y HSL, listos para copiar.
- **Navegar rutas** — `D:\Proy` o `~/Doc` lista la carpeta; Tab autocompleta.
- **Web** — `g gatos`, `yt lofi`, `gh egui`… palabras clave configurables; URLs escritas a mano se abren directo.
- **Terminal** — `>comando` lo ejecuta en una terminal nueva.
- **Sistema** — bloquear, suspender, apagar (pide confirmación), vaciar papelera, páginas de configuración.
- **Aprende de ti** — lo que más abres, y lo que eliges para cada búsqueda, sube en la lista.

Escribe `?` para ver la ayuda dentro de Kero.

## Instalación

Descarga el archivo de tu sistema desde [Releases](https://github.com/AbrahmCh1/kero/releases/latest). Cada release incluye `SHA256SUMS.txt` y una [atestación de procedencia](https://docs.github.com/actions/security-for-github-actions/using-artifact-attestations) que puedes verificar con `gh attestation verify <archivo> --repo AbrahmCh1/kero`.

### Windows 10/11

1. Descomprime `kero-<versión>-windows-x64.zip` (o `arm64`).
2. Clic derecho en `install.ps1` → *Ejecutar con PowerShell*, o:

   ```powershell
   powershell -ExecutionPolicy Bypass -File install.ps1
   ```

Se instala para tu usuario (sin administrador) en `%LOCALAPPDATA%\Programs\Kero`, aparece en el menú Inicio y se desinstala desde *Configuración › Aplicaciones*. Si el ejecutable no está firmado, SmartScreen puede avisar la primera vez.

### macOS 11 o posterior (Apple Silicon e Intel)

Abre `kero-<versión>-macos-universal.dmg` y arrastra **Kero** a Aplicaciones. Si el release no está notarizado, la primera vez abre Kero con clic derecho → *Abrir*.

### Linux (x86_64 y aarch64)

```sh
tar xzf kero-<versión>-linux-x86_64.tar.gz
cd kero-<versión>-linux-x86_64
./install.sh
```

Instala en `~/.local/bin` y añade Kero al menú de aplicaciones. Para desinstalar: `./uninstall.sh`.

En **Wayland** las apps no pueden registrar atajos globales: crea uno en la configuración de tu escritorio que ejecute `kero --toggle`.

## Uso

| Tecla | Acción |
|---|---|
| `Alt+Espacio` (configurable) | Mostrar / ocultar |
| `↑` `↓` `RePág` `AvPág` | Moverse por los resultados |
| `Enter` | Abrir, ejecutar o **copiar** |
| `Ctrl+Enter` · `⌘+Enter` | Mostrar en la carpeta |
| `Ctrl+C` · `⌘+C` | Copiar ruta o resultado |
| `Shift+Enter` | Ejecutar como administrador (Windows) |
| `Tab` | Autocompletar rutas |
| `Esc` | Borrar la búsqueda / ocultar |

| Escribe | Para |
|---|---|
| `chrome`, `factura sat` | Apps, archivos y carpetas |
| `/informe` | Sólo archivos y carpetas |
| `=sqrt(2)` | Forzar la calculadora |
| `>ipconfig` | Ejecutar en terminal |
| `kero` | Reindexar, agregar carpeta, configuración, inicio automático, salir |

Si `Alt+Espacio` ya está ocupado (por ejemplo, por PowerToys Run), Kero usa `Ctrl+Alt+Espacio` y lo avisa.

### Línea de comandos

```
kero            abre Kero (si ya está abierto, lo muestra)
kero --toggle   muestra u oculta la instancia abierta
kero --hidden   arranca en segundo plano
kero --quit     cierra la instancia abierta
kero --version
```

## Configuración

`config.toml` se crea en el primer arranque. Ábrelo escribiendo `kero config`; los cambios se aplican la siguiente vez que muestras Kero.

| Sistema | Ruta |
|---|---|
| Windows | `%APPDATA%\Kero\config.toml` |
| macOS | `~/Library/Application Support/Kero/config.toml` |
| Linux | `~/.config/kero/config.toml` |

```toml
hotkey = "alt+space"     # p. ej. "ctrl+space", "super+alt+k"
autostart = false
theme = "auto"           # auto | dark | light

[index]
roots = ["~/Desktop", "~/Documents", "~/Downloads", "~/Proyectos"]
exclude = ["node_modules", ".git", "target"]
skip_hidden = true
watch = true             # reindexa sólo la carpeta que cambió

[[web]]
keyword = "g"
name = "Google"
url = "https://www.google.com/search?q={q}"
```

Para agregar una carpeta sin editar el archivo: escribe `kero agregar` y elígela.

## Estado por plataforma

| | Windows | macOS | Linux |
|---|:-:|:-:|:-:|
| Apps, archivos, plugins | ✅ | ✅ | ✅ |
| Atajo global | ✅ | ✅ | X11 ✅ · Wayland vía `kero --toggle` |
| Icono en bandeja / barra de menús | ✅ | ✅ | — |
| Iconos reales de apps y archivos | ✅ | iniciales y glifos | iniciales y glifos |
| Ventana con sombra y esquinas | ✅ | panel opaco | panel opaco |
| Inicio automático | ✅ | ✅ (LaunchAgent) | ✅ (XDG autostart) |

Windows es la plataforma principal. macOS y Linux están en **soporte inicial**: se compilan y publican en cada release, pero aún no se han probado a fondo en equipos reales. Los reportes son bienvenidos.

## Compilar desde el código

Requiere Rust estable.

```sh
cargo build --release
```

En Windows, `scripts\install.ps1` compila, firma (si existe el certificado de `scripts\setup-cert.ps1`) e instala. `kero --bench <consulta>` mide indexado y búsqueda en consola (en builds de depuración).

### Arquitectura

| Módulo | Qué hace |
|---|---|
| `index/` | Apps por sistema y archivos con recorrido paralelo. Cada carpeta se guarda una vez y cada archivo es sólo nombre + carpeta (50 000 archivos ≈ 2,6 MB). Caché binaria por raíz y vigilancia con `notify`. |
| `search.rs` | Hilo de búsqueda que descarta consultas obsoletas; ordena por coincidencia difusa (nucleo), calidad de coincidencia y frecuencia de uso. |
| `plugins/` | Un archivo por plugin; todos implementan `Plugin::query`. |
| `raster.rs`, `host.rs` | egui dibujado por CPU. Windows presenta con `UpdateLayeredWindow` (alfa real); macOS y Linux con softbuffer. Sólo se pinta cuando algo cambia. |
| `platform/` | La misma API sobre Win32 y sobre macOS/Linux. |

### Publicar un release

1. Sube `version` en `Cargo.toml` y haz commit.
2. Crea y sube el tag: `git tag v0.2.0 && git push origin v0.2.0`.

El workflow [`release.yml`](.github/workflows/release.yml) compila para Windows (x64, arm64), macOS (universal) y Linux (x86_64, aarch64), y publica el release. Firma automáticamente si existen estos secretos en el repositorio:

| Secreto | Para |
|---|---|
| `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD` | Certificado de firma de código (PFX en base64) |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` | Certificado *Developer ID Application* (P12 en base64) |
| `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD` | Notarización de macOS |

## Licencia

[MIT](LICENSE) © 2026 ACY Solutions

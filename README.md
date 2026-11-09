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
| Memoria en reposo | **≈10 MB** privados; <1 MB de working set tras 10 s oculto |
| CPU en reposo | 0 % (sólo se despierta con el atajo) |
| Búsqueda | **1–2 ms** sobre 50 000 archivos + 250 apps |
| Indexado | ≈0,5 s para 50 000 archivos; arranque instantáneo desde caché |
| Ejecutable | ≈6 MB, sin runtime ni dependencias |

Escrito en Rust. La interfaz se dibuja con un rasterizador propio en CPU, así que no carga drivers de GPU (que en equipos con gráficos híbridos cuestan ~50 MB por sí solos).

## Qué hace

- **Apps** — todas las del sistema (en Windows también las de Microsoft Store).
- **Archivos y carpetas** — búsqueda difusa en las carpetas que elijas; el índice se actualiza solo al crear, borrar o renombrar.
- **Calculadora y unidades** — `1500 * 1.16`, `sqrt(2)`, `5 km to mi`, `0xff to decimal`. **Enter copia el resultado.**
- **Divisas** — `100 usd a mxn`, `50 euros en dólares`, `0.1 btc`; sólo con la moneda (`100 usd`) convierte a la tuya. Tasas diarias de ~200 monedas y cripto, sin cuenta ni clave.
- **Historial del portapapeles** — `cb` muestra lo último que copiaste (sólo texto); `cb factura` lo filtra. Enter lo vuelve a copiar, `Ctrl+S` lo guarda como texto fijo, `Shift+Supr` lo quita.
- **Textos guardados** — firmas, direcciones, respuestas frecuentes: `sn` los lista y también aparecen al buscar su nombre. Viven en `snippets.txt`, editable a mano.
- **Zonas horarias** — `hora en tokio`, `15:00 ny a cdmx`, `3pm madrid` (de Madrid a tu hora), `9am a londres` (de tu hora a Londres). Usa los datos de zonas del propio sistema, con horario de verano.
- **Fechas** — `hoy`, `hoy + 90 días`, `en 3 semanas`, `días hasta 24 dic`, `días desde 1 ene`, `24/12/2026` (qué día cae), `1730592000` (fecha Unix → fecha).
- **Herramientas de texto** — `uuid`, `contraseña 24` (se copia sin quedar en ningún historial), `base64`, `url`, `sha256`/`sha1`/`md5`, `json` (formatea o minifica), `contar`. Sin texto, trabajan sobre lo que tengas copiado.
- **Atajos propios** — en `config.toml`, un nombre que abre una carpeta, archivo o URL, o ejecuta un comando.
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

Con Homebrew:

```sh
brew tap AbrahmCh1/kero https://github.com/AbrahmCh1/kero && brew install --cask AbrahmCh1/kero/kero-launcher
```

El cask se llama `kero-launcher` porque `kero` ya está ocupado por otra app en Homebrew (que también instala `Kero.app`); la app instalada sigue siendo **Kero**. No instales ambos casks a la vez. Para actualizar: `brew upgrade --cask kero-launcher`.

También puedes instalar a mano: abre `kero-<versión>-macos-universal.dmg` y arrastra **Kero** a Aplicaciones.

Desde la 0.2.1 está firmado con Developer ID y notarizado por Apple, así que se abre sin avisos de Gatekeeper.

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
| `Ctrl+N` · `Ctrl+P` | Siguiente / anterior resultado |
| `Ctrl+D` | Borrar el carácter a la derecha |
| `Enter` | Abrir, ejecutar o **copiar** |
| `Ctrl+Enter` · `⌘+Enter` | Mostrar en la carpeta |
| `Shift+Enter` | Archivos: abrir una terminal en su carpeta · apps y `>comandos`: como administrador (Windows) |
| `Ctrl+Shift+Enter` | "Abrir con…" (Windows) |
| `Ctrl+C` · `⌘+C` | Copiar ruta o resultado |
| `Ctrl+Shift+C` · `⌘+⇧+C` | Copiar el archivo en sí, para pegarlo en una carpeta, un correo o un chat |
| `Ctrl+S` · `⌘+S` | En el historial del portapapeles: guardarlo como texto fijo |
| `Shift+Supr` · `⇧+⌫` | Quitar del historial o borrar un texto guardado |
| `Tab` | Autocompletar rutas |
| `Esc` | Borrar la búsqueda / ocultar |

En macOS, dentro del texto, `Ctrl+A` va al inicio, `Ctrl+E` al final, `Ctrl+B` retrocede y `Ctrl+F` avanza. En Windows y Linux, `Ctrl+A` selecciona todo.

| Escribe | Para |
|---|---|
| `chrome`, `factura sat` | Apps, archivos y carpetas |
| `/informe` | Sólo archivos y carpetas |
| `=sqrt(2)` | Forzar la calculadora |
| `100 usd a mxn` | Convertir divisas |
| `cb`, `cb texto` | Historial del portapapeles |
| `sn`, `sn texto` | Textos guardados |
| `hora en tokio`, `15:00 ny a cdmx` | Zonas horarias |
| `hoy + 90 días`, `días hasta 24 dic` | Fechas |
| `uuid`, `contraseña`, `sha256 texto`, `json` | Herramientas de texto |
| `>ipconfig` | Ejecutar en terminal |
| `kero` | Reindexar, agregar carpeta, configuración, inicio automático, salir |

Si `Alt+Espacio` ya está ocupado (por ejemplo, por PowerToys Run), Kero usa `Ctrl+Alt+Espacio` y lo avisa.

### Desde la terminal

El instalador agrega `kero` al PATH (abre una terminal nueva después de instalar). En macOS, el cask de Homebrew ya lo enlaza. Si instalaste el DMG a mano, enlázalo una vez: `sudo ln -s /Applications/Kero.app/Contents/MacOS/kero /usr/local/bin/kero`.

```
kero                       abre Kero (si ya está abierto, lo muestra u oculta)
kero factura sat           abre Kero con esa búsqueda

kero buscar <texto> [-n N] lista resultados: apps, archivos, comandos
kero abrir <texto>         abre el primer resultado
kero calc <expresión>      2^10 · 5 km to mi · 100 usd a mxn
                           también: hora en tokio · hoy + 90 días · uuid · sha256 hola
kero tasas                 actualiza y muestra los tipos de cambio
kero clip                  historial del portapapeles, numerado
kero clip 3                vuelve a copiar la entrada 3
kero clip <texto>          filtra el historial · kero clip borrar lo vacía
kero snip [n | nombre]     textos guardados: los lista, o copia uno
kero copiar [texto]        copia el texto o lo que llegue por la tubería
kero pegar                 escribe lo que hay en el portapapeles
kero reindexar             vuelve a indexar
kero mostrar · salir       muestra/oculta o cierra la instancia abierta
kero config · version · ayuda
```

También en inglés: `search`, `open`, `rates`, `copy`, `paste`, `reindex`, `toggle`, `quit`, `help`. Con una tubería la salida es sólo datos, para encadenar:

```powershell
kero buscar informe | Select-Object -First 1 | ForEach-Object { code $_ }
Get-Content notas.txt | kero copiar
$total = kero calc 1500 usd a mxn # sólo el número, con las tasas del día
```

En Windows, `kero.com` (consola) va junto a `kero.exe` (ventana): la terminal elige `kero.com`, que espera la respuesta y muestra la salida.

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
currency = "auto"        # moneda local: "MXN", "EUR"… o la de tu región
persist_query = false    # conserva la búsqueda al ocultar y la deja seleccionada al reabrir

[clipboard]
enabled = true           # historial del portapapeles (sólo texto)
max_items = 50
hotkey = ""              # atajo directo al historial, p. ej. "ctrl+shift+v"

[index]
roots = ["~/Desktop", "~/Documents", "~/Downloads", "~/Proyectos"]
exclude = ["node_modules", ".git", "target"]
skip_hidden = true
watch = true             # reindexa sólo la carpeta que cambió

[[web]]
keyword = "g"
name = "Google"
url = "https://www.google.com/search?q={q}"

[[shortcut]]                 # atajos propios: aparecen al buscar su nombre
name = "Proyectos"
open = "D:\\Proyectos"       # carpeta, archivo, app o URL

[[shortcut]]
name = "Actualizar repos"
run = "git -C ~/kero pull"   # se ejecuta en una terminal
```

`persist_query` mantiene en memoria lo que escribiste al ocultar Kero y, al reabrir, lo deja seleccionado para reemplazarlo o continuar. No se guarda en disco.

Para agregar una carpeta sin editar el archivo: escribe `kero agregar` y elígela.

Los textos guardados están en `snippets.txt`, junto a `config.toml`. Cada uno empieza con `## nombre` y sigue con el texto, que puede tener varias líneas:

```
## firma
Abraham Chávez
ACY Solutions
```

### Privacidad

- **Divisas:** la única conexión de Kero a internet. Descarga las tasas del día de [exchange-api](https://github.com/fawazahmed0/exchange-api) (vía jsDelivr, con copia en Cloudflare) sólo cuando escribes una conversión y las guardadas tienen más de 12 h. No envía lo que escribes.
- **Portapapeles:** el historial se guarda en tu equipo (`clipboard.txt` en la carpeta de datos de Kero) y se borra al desinstalar. En Windows y macOS no entra lo que los gestores de contraseñas marcan como privado (1Password, Bitwarden, KeePass…). En Linux no hay una marca común: si copias contraseñas a menudo, desactívalo con `[clipboard] enabled = false` o vacíalo con `kero clip borrar`.
- **Contraseñas generadas:** salen del generador aleatorio del sistema y se copian marcadas como privadas: no entran al historial de Kero, ni al de Windows (`Win+V`) ni a la nube, ni a los gestores de portapapeles de macOS que respetan la marca.

## Estado por plataforma

| | Windows | macOS | Linux |
|---|:-:|:-:|:-:|
| Apps, archivos, plugins | ✅ | ✅ | ✅ |
| Atajo global | ✅ | ✅ | X11 ✅ · Wayland vía `kero --toggle` |
| Icono en bandeja / barra de menús | ✅ | ✅ | — |
| Iconos reales de apps y archivos | ✅ | ✅ apps, archivos y carpetas | iniciales y glifos |
| Ventana con sombra y esquinas | ✅ | panel opaco | panel opaco |
| Inicio automático | ✅ | ✅ (LaunchAgent) | ✅ (XDG autostart) |
| Historial del portapapeles | ✅ (por eventos, respeta gestores de contraseñas) | ✅ (contador del sistema, respeta gestores de contraseñas) | ✅ (consulta cada 1 s) |
| `kero` en la terminal | ✅ (en el PATH) | Homebrew ✅ · DMG enlace manual | ✅ (`~/.local/bin`) |

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
| `rates.rs`, `clipboard.rs`, `snippets.rs` | Tipos de cambio (descarga perezosa y caché en disco), historial del portapapeles y textos guardados. |
| `tz.rs`, `digest.rs` | Fechas civiles y zonas horarias (ICU de Windows, que se carga al primer uso; zoneinfo en macOS/Linux) y MD5/SHA, sin dependencias. |
| `cli.rs`, `bin/kero-console.rs` | Subcomandos de terminal; en Windows, la consola `kero.com` que los reenvía a `kero.exe`. |
| `raster.rs`, `host.rs` | egui dibujado por CPU. Windows presenta con `UpdateLayeredWindow` (alfa real); macOS y Linux con softbuffer. Sólo se pinta cuando algo cambia. |
| `platform/` | La misma API sobre Win32 y sobre macOS/Linux. |

### Publicar un release

1. Sube `version` en `Cargo.toml` y haz commit.
2. Crea y sube el tag: `git tag v0.2.1 && git push origin v0.2.1`.

El workflow [`release.yml`](.github/workflows/release.yml) compila para Windows (x64, arm64), macOS (universal) y Linux (x86_64, aarch64), y publica el release. Después actualiza `Casks/kero-launcher.rb` en `master` con la versión y el SHA-256 del DMG. Si `master` está protegida, permite escritura al `GITHUB_TOKEN` o usa un PAT. Firma automáticamente si existen estos secretos en el repositorio:

| Secreto | Para |
|---|---|
| `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD` | Certificado de firma de código (PFX en base64) |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` | Certificado *Developer ID Application* (P12 en base64) |
| `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD` | Notarización de macOS |

## Licencia

[MIT](LICENSE) © 2026 ACY Solutions

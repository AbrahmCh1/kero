# Kero

Lanzador estilo Spotlight para Windows, escrito en Rust. Ligero (≈12 MB de memoria en reposo, 0 % de CPU), búsqueda en ~1–2 ms y un índice de archivos compacto que se actualiza solo.

## Uso

| Tecla | Acción |
|---|---|
| `Alt+Espacio` (configurable) | Mostrar / ocultar |
| `↑` `↓` `RePág` `AvPág` | Moverse por los resultados |
| `Enter` | Abrir / ejecutar / **copiar** (calculadora, colores) |
| `Ctrl+Enter` | Abrir la carpeta que contiene el archivo o la app |
| `Shift+Enter` | Ejecutar como administrador |
| `Ctrl+C` | Copiar ruta o resultado (si no hay texto seleccionado) |
| `Tab` | Autocompletar rutas |
| `Esc` | Borrar la búsqueda / ocultar |

Si PowerToys Run ya ocupa `Alt+Espacio`, Kero usa `Ctrl+Alt+Espacio` y lo avisa. Desactiva PowerToys Run o cambia `hotkey` en la configuración.

### Qué puedes escribir

| Ejemplo | Plugin |
|---|---|
| `chrome`, `factura sat` | Apps (incluidas las de Microsoft Store), archivos y carpetas |
| `/informe` | Sólo archivos y carpetas |
| `1500 * 1.16`, `5 km to mi`, `0xff to decimal` | Calculadora y conversor de unidades (fend) |
| `=sqrt(2)` | Forzar la calculadora |
| `D:\Proy` | Navegar carpetas |
| `#3b82f6`, `rgb(59,130,246)` | Convertir colores |
| `g gatos`, `yt lofi`, `gh egui` | Búsqueda web por palabra clave |
| `github.com/rust-lang` | Abrir una URL |
| `>ipconfig` | Ejecutar en una terminal |
| `bloquear`, `apagar`, `bluetooth`, `wifi` | Comandos del sistema y Configuración de Windows |
| `kero` | Reindexar, agregar carpeta, configuración, inicio con Windows |
| `?` | Ayuda |

Kero aprende de tu uso: lo que abres más y lo que eliges para cada búsqueda sube en la lista.

## Configuración

`%APPDATA%\Kero\config.toml` (se crea al primer arranque; los cambios se aplican la siguiente vez que abres Kero):

```toml
hotkey = "alt+space"
autostart = false
theme = "auto"        # auto | dark | light

[index]
roots = ["~\\Desktop", "~\\Documents", "~\\Downloads", "D:\\Proyectos"]
exclude = ["node_modules", ".git", "target"]
skip_hidden = true
watch = true          # reindexa sola la carpeta que cambió

[[web]]
keyword = "g"
name = "Google"
url = "https://www.google.com/search?q={q}"
```

Para agregar carpetas sin editar el archivo: escribe `kero agregar` y elige la carpeta.

## Instalar

```powershell
powershell -ExecutionPolicy Bypass -File scripts\install.ps1
```

Compila, firma (si hay certificado) e instala Kero por usuario, sin permisos de administrador, en `%LOCALAPPDATA%\Programs\Kero`. Queda en el menú Inicio y en Configuración › Aplicaciones (editor: ACY Solutions), desde donde se desinstala. Volver a ejecutarlo actualiza la instalación.

### Firma (sólo para este equipo)

Una sola vez, para que Windows muestre "ACY Solutions" en lugar de "Editor desconocido":

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup-cert.ps1
```

Crea un certificado autofirmado de firma de código (clave no exportable) y lo marca como de confianza sólo para tu usuario; Windows pide confirmarlo. En otros equipos la firma no será de confianza: para distribuir hace falta un certificado comercial. Después, `install.ps1` firma automáticamente.

La instalación vive en scripts y no dentro de `kero.exe` a propósito: un exe sin firmar que se copia a sí mismo y se registra dispara la heurística de Defender (`Bearfoos.A!ml`).

## Desarrollo

`cargo build --release` deja el ejecutable en `target/release/kero.exe`. `kero.exe --bench chrome` mide el indexado y la búsqueda en consola (sólo en builds de depuración, que tienen consola). `kero.exe --quit` cierra la instancia abierta.

## Arquitectura

- `index/` — apps desde `shell:AppsFolder` y archivos con un recorrido paralelo (rayon). Cada carpeta se guarda una vez y cada archivo es sólo nombre + índice de carpeta: ~50k archivos ≈ 2.6 MB. Caché binaria por carpeta raíz para arrancar al instante; `notify` vigila altas, bajas y renombres y reindexa sólo la raíz afectada.
- `search.rs` — hilo de búsqueda que descarta consultas obsoletas; los plugins empujan resultados y se ordenan por puntuación (nucleo + calidad de coincidencia + frecuencia de uso).
- `plugins/` — un archivo por plugin, todos implementan `Plugin::query`.
- `raster.rs` + `host.rs` — egui renderizado **por CPU** y presentado con `UpdateLayeredWindow`. Así no se carga ningún driver OpenGL/DirectX (≈50 MB menos en equipos con GPU híbrida), la ventana tiene transparencia real y los píxeles vacíos dejan pasar los clics. Un fotograma cuesta ~4 ms y sólo se pinta cuando algo cambia.
- Al ocultarse 10 s, Kero devuelve su memoria de trabajo al sistema (`EmptyWorkingSet`).

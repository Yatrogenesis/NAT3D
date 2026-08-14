# NAT3D en Termux (Android)

**NO PROBADO EN DISPOSITIVO.** Este procedimiento está derivado de las
dependencias reales declaradas en los `Cargo.toml` del workspace (verificadas
2026-08-09), no de una instalación ejecutada. Los puntos que hay que confirmar
en el primer intento están marcados con **[VERIFICAR]**.

---

## Antes de nada: Termux ≠ APK de Android

Son dos destinos distintos y NAT3D ya contempla los dos:

| Quieres… | Usa | Por qué |
|---|---|---|
| Correr NAT3D **desde la terminal** del teléfono | `nat3d-cli` (este documento) | No necesita display, ni GPU, ni X11 |
| Una **app con ventana** y aceleración gráfica | `nat3d-mobile` → APK con NDK | Ya declara `android-activity` + `winit/android-native-activity` |

`nat3d-app` (la GUI de escritorio, 23k LOC sobre `eframe`+`wgpu`) **no es el
camino en Termux**: exigiría `termux-x11` más un ICD de Vulkan experimental, y
compilar `wgpu` en el propio teléfono. Si lo que quieres es GUI en Android, el
camino correcto es el APK, no Termux.

---

## Requisitos

- Termux instalado desde **F-Droid o GitHub**, no de Google Play (la versión de
  Play está congelada y sus paquetes rompen)
- ~4 GB libres (el directorio `target/` de Rust crece mucho)
- Compilar tarda: en un teléfono de gama media, decenas de minutos

---

## Instalación

```bash
# 1. Base
pkg update && pkg upgrade -y
pkg install -y rust git binutils build-essential pkg-config

# 2. Clonar (repo privado: usa un Personal Access Token cuando pida contraseña)
git clone https://github.com/Yatrogenesis/NAT3D.git
cd NAT3D

# 3. Compilar SOLO lo que corre en terminal.
#    -j 1 es deliberado: el enlazador es lo que más RAM consume y Android
#    mata procesos por memoria sin avisar. Con paralelismo alto, el build
#    muere a la mitad y el error no dice que fue el OOM killer.
CARGO_BUILD_JOBS=1 cargo build --release -p nat3d-cli

# 4. Ejecutar
./target/release/nat3d --help
./target/release/nat3d render entrada.obj --output salida.png
```

---

## Si algo falla

**El build muere sin mensaje claro** → casi siempre es el OOM killer de Android.
Cierra apps, confirma `CARGO_BUILD_JOBS=1`, y considera `termux-wake-lock` para
que el sistema no suspenda el proceso.

**Error compilando `ring` u `openssl`** → alguna dependencia transitiva quiere
criptografía nativa:
```bash
pkg install -y openssl clang
export OPENSSL_DIR=$PREFIX
```
**[VERIFICAR]** si aparece: el workspace declara `tokio` con `features=["full"]`,
que por sí solo no arrastra TLS, pero `nat3d-cloud`/`nat3d-sync` podrían.
Si estorban, no se compilan aquí — este build solo incluye `cli` y `tui`.

**`cargo` no encuentra el linker** → `pkg install binutils` (ya está en el paso 1).

**Sobre la TUI** → `nat3d-tui` está suspendida y no se construye; si la compilas
a mano desde su manifiesto, ten presente que siete de sus ocho acciones no hacen
nada. Cuando se reactive conviene un teclado con
teclas de función. Termux:Styling ayuda con la fuente.

---

## Qué esperar de cada uno

- **`nat3d-cli`** (135 líneas): interfaz de línea de comandos. Implementa **un
  subcomando, `render`**. Es lo único de esta ruta que hace trabajo real hoy.
- **`nat3d-tui`** (378 líneas): **suspendida, y excluida del build.** Su menú
  ofrece ocho acciones y siete de ellas invocan subcomandos que `nat3d-cli` no
  implementa; además resuelve el binario como `nat3d.exe`, un nombre de Windows,
  cuando el binario se llama `nat3d`. El crate sigue en el árbol con la
  explicación en `crates/nat3d-tui/README.md`, pero no se compila ni se
  distribuye. No la instales esperando un menú funcional.

Ninguno de los dos renderiza 3D acelerado. Para eso está el APK.

---

## Para el APK (la otra ruta, no cubierta aquí)

`nat3d-mobile` ya está preparado. Necesitarías, **desde un PC, no desde el
teléfono**: Android NDK, `rustup target add aarch64-linux-android`, y
`cargo-apk` o `xbuild`. Es un procedimiento distinto y merece su propio
documento.

---

SPDX-License-Identifier: AGPL-3.0-or-later · (C) 2026 Francisco Molina-Burgos
</content>

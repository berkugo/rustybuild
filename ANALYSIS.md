# ngmake vs CMake + Ninja/Make — Eksiklikler, TOML Referansı ve Öneriler

---

## 📋 build.toml Referansı (Desteklenen Parametreler ve Kurallar)

Uygulama TOML dosyasını parse ederken aşağıdaki yapı ve alanları kullanır. Bu kurallara uyan bir `build.toml` yazıldığında veya CMake’ten dönüştürüldüğünde build doğru çalışır.

### Üst seviye: `[project]`

| Alan | Zorunlu | Varsayılan | Açıklama |
|------|--------|------------|----------|
| `name` | Hayır | `"unnamed_project"` | Proje adı (bilgi amaçlı). |
| `version` | Hayır | `"0.1.0"` | Proje sürümü (bilgi amaçlı). |
| `cxx_standard` | Hayır | — | **Sadece root** build.toml'da okunur. Tüm proje için C++ standardı (11, 14, 17, 20, 23). Alt TOML'lardaki target cxx_standard **override edilemez**. |
| `includes` | Hayır | `[]` | Başka `build.toml` dosyalarının yolları (bu dosyaya göre relative). Parse sırasında bu dosyalar da okunur ve target’lar tek projede birleştirilir. Cross-module bağımlılık için kullanılır. |

**Örnek:**
```toml
[project]
name = "my_app"
version = "1.0.0"
cxx_standard = 17
includes = ["libs/utils/build.toml", "libs/core/build.toml"]
```

### Alt dosyalar (includes): `[module]`

Root tarafından `includes` ile eklenen build.toml dosyalarında **`[project]` değil `[module]`** kullanılır. Proje bilgisi (version, cxx_standard) sadece root’ta vardır; modülde sadece isim ve kendi includes’ı tanımlanır.

| Alan | Zorunlu | Varsayılan | Açıklama |
|------|--------|------------|----------|
| `name` | Hayır | `"unnamed_module"` | Modül adı (bilgi / log amaçlı). |
| `includes` | Hayır | `[]` | Bu modülün bağlı olduğu diğer modül build.toml yolları. |

**Örnek (libs/core/build.toml):**
```toml
[module]
name = "core"
includes = ["../utils/build.toml"]

[[target]]
name = "core_base"
type = "static_lib"
...
```

Geriye uyumluluk: Alt dosyada `[project]` yazılırsa da okunur (module gibi; `version` ve `cxx_standard` yok sayılır). Yeni dosyalarda `[module]` kullanılması önerilir.

### Hedefler: `[[target]]`

Her `[[target]]` bloğu tek bir build hedefini tanımlar (executable, static lib, shared lib). Target isimleri proje genelinde benzersiz olmalıdır (root + tüm `includes` içinde).

| Alan | Zorunlu | Varsayılan | Açıklama |
|------|--------|------------|----------|
| `name` | **Evet** | — | Benzersiz hedef adı. Link/bağımlılık için kullanılır; `deps` içinde bu isimler geçer. |
| `type` | Hayır | `"executable"` | Hedef türü: `"executable"`, `"static_lib"`, `"shared_lib"`. |
| `sources` | Hayır | `[]` | Kaynak dosya listesi. Öğeler dosya yolu veya glob pattern olabilir (örn. `src/**/*.cpp`). Yollar `build.toml` dosyasının bulunduğu dizine göre çözülür. |
| `include_dirs` | Hayır | `[]` | Derleyici için `-I` dizinleri (relative veya absolute). Derleme sırasında dependency’lerin `include_dirs` değerleri de eklenir (propagation). |
| `lib_dirs` | Hayır | `[]` | Linker için `-L` dizinleri. Harici kütüphanelerin `.a`/`.so` dosyalarının bulunduğu dizinler. |
| `libs` | Hayır | `[]` | Link edilecek kütüphane isimleri (`-l` ile verilen, örn. `"pthread"`, `"m"`). Sadece harici sistem kütüphaneleri için; proje içi target’lar `deps` ile verilir. |
| `flags` | Hayır | `[]` | Eski/ortak bayraklar: sadece **derleme** aşamasında kullanılır (geriye uyumluluk). Yeni projelerde `compiler_flags` / `linker_flags` tercih edilir. |
| `cxx_standard` | Hayır | — | C++ standardı (sayı: 11, 14, 17, 20, 23). **Root [project]'te `cxx_standard` tanımlıysa tüm target'lar onu kullanır; bu alan yok sayılır.** Sadece root'ta [project] cxx_standard yoksa target seviyesi kullanılır. |
| `compiler_flags` | Hayır | `[]` | Sadece **derleme** aşamasında kullanılan bayraklar (örn. `-O2`, `-Wall`). Link aşamasına geçmez. Dependency'lerden propagate edilir. |
| `linker_flags` | Hayır | `[]` | Sadece **link** aşamasında kullanılan bayraklar (örn. `-Wl,--as-needed`). Derleme aşamasına geçmez. Dependency'lerden propagate edilir. |
| `deps` | Hayır | `[]` | Bu hedefin bağımlı olduğu **proje içi** target isimleri. Sıra link sırasını etkiler; DAG’e göre önce dependency’ler derlenir/link edilir. Sadece aynı projede (root + includes) tanımlı target isimleri geçerlidir. |
| `compiler` | Hayır | `"g++"` | Kullanılacak derleyici: `"gcc"`, `"g++"`, `"clang"`. |
| `output_dir` | Hayır | `"build"` | Bu hedefin çıktılarının yazılacağı dizin (object, .a/.so veya executable). `build.toml` dizinine göre relative. |

**Örnek:**
```toml
[[target]]
name = "myapp"
type = "executable"
sources = ["src/main.cpp", "src/app.cpp"]
include_dirs = ["include"]
deps = ["mylib_shared"]
cxx_standard = 17
compiler_flags = ["-O2", "-Wall"]
linker_flags = ["-Wl,--as-needed"]

[[target]]
name = "mylib_shared"
type = "shared_lib"
sources = ["src/mylib.cpp"]
include_dirs = ["include"]
output_dir = "build"
```

### Kurallar ve davranış

- **Yol çözümleme:** Tüm relative yollar (sources, include_dirs, lib_dirs, output_dir), `build.toml` dosyasının bulunduğu dizine göre çözülür. Include edilen her `build.toml` kendi dizini ile çözülür.
- **Glob:** `sources` içinde `*`, `?`, `[ ]` kullanılırsa glob pattern olarak işlenir; eşleşen dosyalar listelenir.
- **Duplicate target:** Aynı isimde target birden fazla `build.toml`’da (veya includes ile) gelirse **ilk tanım geçerli** olur, sonrakiler yok sayılır.
- **Dependency propagation:** Bir target’ın `deps` listesindeki her isim için, o dependency’nin `include_dirs`, `libs`, `flags`, `compiler_flags`, `linker_flags` değerleri bu target’a eklenir (transitif değil, sadece doğrudan dep’ler; propagation bir sonraki aşamada uygulanır).
- **TOML yapısı:** Root’ta `[project]`, include edilen dosyalarda `[module]`; hedefler her yerde `[[target]]` ile tanımlanır. `[[target]]` birden fazla kez kullanılabilir.

### Tek root (workspace root) — CMake benzeri

Derleme **her zaman tek root** üzerinden yapılır. Verilen `build.toml` dosyası, başka bir (üst) `build.toml` tarafından `includes` ile listeleniyorsa, uygulama **workspace root**'u bulur ve projeyi oradan yükler. Root build.toml tek giriş noktasıdır; alt build.toml'lar sadece include edilir. Alt dosyadan hedef seçilse bile build root'tan alınır. Root tespiti `parse_build_file()` içinde otomatik uygulanır.

### Desteklenmeyen (henüz)

- `definitions` (ayrı alan yok; `-DXXX` için `compiler_flags` kullanın)
- `build_type` (debug/release) — tek konfigürasyon
- `type = "interface"` veya `"object_lib"` — sadece executable, static_lib, shared_lib

---

## 🧪 Önerilen CMake Projesi (Dönüşüm Denemesi İçin)

Aşağıdaki projeleri **elle** veya **ngmake GUI’deki “Convert CMake”** ile `build.toml` yapısına dönüştürebilirsiniz.

1. **CMake resmi öğretici (Steps 1–3)**  
   - [CMake Tutorial](https://cmake.org/cmake/help/latest/guide/tutorial/index.html) — Step 1–3: basit executable, MathFunctions static lib, use of `add_subdirectory`, `target_include_directories`, `target_link_libraries`.  
   - Küçük ve anlaşılır; dönüşüm kurallarını test etmek için idealdir.

2. **spdlog**  
   - GitHub: `https://github.com/gabime/spdlog`  
   - Header-only + opsiyonel compiled lib; CMake ile birkaç target. Orta seviye karmaşıklık.

3. **fmt**  
   - GitHub: `https://github.com/fmtlib/fmt`  
   - Genelde tek bir kütüphane target’ı ve birkaç örnek executable. Dönüşüm için uygun.

4. **nlohmann/json**  
   - GitHub: `https://github.com/nlohmann/json`  
   - Çoğunlukla header-only; CMake’te interface/optional single header. Basit bir test projesi olarak kullanılabilir.

**Öneri:** Önce CMake Tutorial Step 1–3’ü indirip (veya kendi kopyanızı oluşturup) Convert CMake ile `build.toml` üretin; sonra `ngm build -c build.toml` ile derleyip çıktıyı karşılaştırın. Eksik kalan include veya link’leri `build.toml` referansına göre elle tamamlayabilirsiniz.

---

## 📊 CMake + Ninja'ya Göre Eksiklik Özeti

ngmake şu an tek konfigürasyonlu, tek grafikli bir build aracı. CMake + Ninja ile karşılaştırıldığında eksik veya farklı olanlar:

| Konu | CMake + Ninja | ngmake |
|------|----------------|--------|
| **Build type** | Debug / Release / RelWithDebInfo | Yok (tek config) |
| **Definitions** | `target_compile_definitions()` | Sadece `flags` içinde `-D...` |
| **C++ standard** | `CMAKE_CXX_STANDARD` / `target_compile_features` | Sadece `flags` içinde `-std=c++17` |
| **Compiler** | Otomatik tespit | Manuel `compiler` alanı |
| **Linker flags** | `target_link_options()` | Sadece `flags` içinde `-Wl,...` |
| **Install** | `install(TARGETS/FILES)` | Yok |
| **Test** | CTest, `add_test()` | Yok |
| **Cross-compile** | Toolchain file | Yok |
| **Find package** | `find_package()` | Yok |
| **Interface / Object / Imported target** | Var | Sadece executable, static_lib, shared_lib |
| **Custom commands/targets** | `add_custom_command/target` | Yok |
| **RPATH** | `INSTALL_RPATH` vb. | Kısmen (LD_LIBRARY_PATH) |
| **Workspace** | Tek root, tüm target'lar tek grafikte | Her `build.toml` + `includes` ile; cross-module için leaf'lere `includes` eklenmeli |

Detaylı madde madde eksiklikler ve öneriler aşağıdaki bölümlerde (Kritik / Önemli / İyileştirme) listelenmiştir.

---

## 📦 Workspace ve Cross-Module Bağımlılıklar (ngmake vs CMake)

### Sorun: "Unknown dependency" when building from a leaf build.toml

**Belirti:** GUI'de örn. `libs/security/build.toml` seçiliyken Build çalıştırılınca:  
`Target 'security_shared' has unknown dependency 'utils_shared'. Defined targets: ["security_crypto", "security_shared"]`

**Sebep:** Build, seçilen tek bir `build.toml` ile çalışır. O dosyada tanımlı olmayan (başka modülde tanımlı) target'lar yüklenmez; `deps` içindeki isimler "defined targets" listesinde yoksa DAG aşamasında hata verir. Bu davranış CMake'deki "tek root, tek grafik" modelinden farklıdır.

### CMake / Ninja / Make bu durumu nasıl çözüyor?

| Araç | Model | Cross-module nasıl çözülür? |
|------|--------|-----------------------------|
| **CMake** | Tek proje (single configuration). Genelde **root'tan** configure: `cmake -B build .` | Root `CMakeLists.txt` tüm alt dizinleri `add_subdirectory(libs/utils)`, `add_subdirectory(libs/security)` ile ekler. Tüm target'lar **tek global scope**'ta; `security` target'ı `utils`'e link ederken `utils` zaten projede tanımlı. Alt dizinden tek başına configure etmek nadirdir; yapılsa bile o dizindeki `CMakeLists.txt` genelde üst dizini veya gerekli modülleri include eder. |
| **Ninja** | CMake veya başka generator'ın ürettiği tek `build.ninja` dosyası. | Tüm target'lar tek dosyada; cross-module zaten tek grafikte. |
| **GNU Make** | Genelde tek root `Makefile` veya `include` ile alt makefile'lar. | Tüm hedefler tek make grafiğinde; bağımlılıklar tek yerde çözülür. |

Özet: CMake/Ninja/Make'de **tek konfigürasyon, tek hedef grafiği** vardır; cross-module bağımlılık "hangi dosyayı açtığına" göre değişmez. ngmake'de ise **hangi build.toml ile build ettiğin** önemli: sadece o dosya + onun `includes`'ı yüklenir.

### ngmake'de çözüm: `includes` ile bağımlı modülleri yükleme

Cross-module bağımlılığı olan her **leaf** `build.toml` dosyasında, bağımlı olduğu modüllerin `build.toml` dosyalarını `[project] includes` ile eklemek gerekir. Böylece o dosyadan build alındığında tüm gerekli target'lar yüklenir.

**Örnek:** `libs/security/build.toml` → `security_shared` → `utils_shared` bağımlı.

```toml
[project]
name = "security"
version = "1.0.0"
includes = ["../utils/build.toml"]

[[target]]
name = "security_shared"
type = "shared_lib"
deps = ["security_crypto", "utils_shared"]
# ...
```

- Root'tan build (örn. `example_too_complex/build.toml`): Zaten tüm modüller root'un `includes` listesinde → sorun yok.
- Sadece `libs/security/build.toml` ile build (GUI'de bu dosya seçili): Bu dosyada `includes = ["../utils/build.toml"]` varsa parse aşamasında `utils` da yüklenir → `utils_shared` tanımlı olur → "unknown dependency" hatası kalkar.

### ngmake vs CMake: Eksik / Farklı olanlar (bu konuda)

| Konu | CMake | ngmake | Not |
|------|--------|---------|-----|
| **Workspace / root kavramı** | Tek root CMakeLists.txt; configure hep root'tan. | Birden fazla “root” olabilir: her `build.toml` kendi includes’ı ile bağımsız build edilebilir. | ngmake'de “workspace root” zorunlu değil; her modül kendi includes’ı ile self-contained olabilir. |
| **Cross-module dep** | `add_subdirectory` veya `find_package` ile aynı projede / harici projede tanımlı target. | Aynı projede: `includes = ["../other/build.toml"]`. Harici: henüz find_package / imported target yok. | Leaf build.toml'ların `includes` ile kendi bağımlılıklarını declare etmesi gerekir. |
| **GUI / “current file” ile build** | Genelde tüm proje root’tan build; “current file” sadece editör bağlamı. | Build = seçilen build.toml; bu dosyada + includes’ta olmayan target “unknown”. | Bu yüzden cross-module kullanan her build.toml’a ilgili `includes` eklendi. |

### İsteğe bağlı iyileştirme: Workspace root detection (GUI)

cd .

---

## 🔴 Kritik Eksiklikler

### 1. **Build Configurations (Debug/Release)**
**CMake'de:** `CMAKE_BUILD_TYPE` (Debug, Release, RelWithDebInfo, MinSizeRel)
**ngmake'de:** ❌ Yok

**Sorun:** Tek bir build configuration var. Debug ve Release build'leri ayrı yapılamıyor.

**Öneri:**
```toml
[project]
build_type = "debug"  # veya "release", "relwithdebinfo", "minsizerel"

# veya CLI'den:
oximake --build-type release
```

**Etkisi:**
- Debug: `-g -O0`
- Release: `-O3 -DNDEBUG`
- Farklı output dizinleri: `build/debug/`, `build/release/`

### 2. **Preprocessor Definitions (Ayrı Field)**
**CMake'de:** `target_compile_definitions(target PRIVATE MY_DEF=1)`
**ngmake'de:** ❌ Sadece `flags` içinde `-DMY_DEF=1` olarak

**Sorun:** Definitions flags içinde karışıyor, ayrı yönetilemiyor.

**Öneri:**
```toml
[[target]]
name = "mylib"
definitions = ["MY_DEF=1", "VERSION=2.0"]  # -D otomatik eklenir
```

### 3. **C++ Standard (Ayrı Field)**
**CMake'de:** `set(CMAKE_CXX_STANDARD 17)` veya `target_compile_features()`
**ngmake'de:** ❌ Sadece `flags` içinde `-std=c++17`

**Sorun:** Standard flags içinde kayboluyor, otomatik detection yok.

**Öneri:**
```toml
[project]
cxx_standard = 17  # veya 11, 14, 17, 20, 23

# veya per-target:
[[target]]
name = "mylib"
cxx_standard = 20
```

### 4. **Compiler Detection**
**CMake'de:** Otomatik detect eder (gcc, clang, msvc)
**ngmake'de:** ❌ Manuel belirtilmeli

**Sorun:** Her target için manuel compiler seçimi gerekiyor.

**Öneri:**
```toml
[project]
default_compiler = "g++"  # veya "clang++", otomatik detect
```

### 5. **Linker Flags (Ayrı Field)**
**CMake'de:** `target_link_options()`, `target_link_directories()`
**ngmake'de:** ❌ Sadece `flags` içinde `-Wl,...`

**Sorun:** Compiler ve linker flags karışıyor.

**Öneri:**
```toml
[[target]]
name = "mylib"
linker_flags = ["-Wl,--as-needed", "-Wl,-rpath,$ORIGIN"]
```

### 6. **Install Rules**
**CMake'de:** `install(TARGETS ...)`, `install(FILES ...)`
**ngmake'de:** ❌ Yok

**Sorun:** Build edilen dosyalar manuel kopyalanmalı.

**Öneri:**
```toml
[[target]]
name = "myapp"
install = { 
    type = "target",
    destination = "bin"
}

[[install]]
type = "file"
source = "config.json"
destination = "etc"
```

### 7. **Test Framework**
**CMake'de:** `enable_testing()`, `add_test()`, CTest
**ngmake'de:** ❌ Yok

**Sorun:** Test'ler manuel çalıştırılmalı.

**Öneri:**
```toml
[[target]]
name = "test_math"
type = "executable"
test = true  # Test olarak işaretle

# CLI:
oximake test  # Tüm test target'larını çalıştır
```

### 8. **Cross-Compilation**
**CMake'de:** Toolchain files, `CMAKE_SYSTEM_NAME`
**ngmake'de:** ❌ Yok

**Sorun:** Farklı platformlar için build yapılamıyor.

**Öneri:**
```toml
[project]
toolchain = "arm-linux-gnueabihf"
# veya
[project]
target_arch = "aarch64"
target_os = "linux"
```

## 🟡 Önemli Eksiklikler

### 9. **Find Packages / Dependency Management**
**CMake'de:** `find_package(Boost)`, `find_package(OpenSSL)`
**ngmake'de:** ❌ Yok

**Sorun:** External library'ler manuel bulunmalı.

**Öneri:**
```toml
[dependencies]
boost = { version = "1.82", components = ["system", "filesystem"] }
openssl = { version = "3.0" }
```

### 10. **Interface Libraries**
**CMake'de:** `add_library(mylib INTERFACE)`
**ngmake'de:** ⚠️ Kısmi (INTERFACE propagation var ama tam destek yok)

**Sorun:** Header-only library'ler için tam destek yok.

**Öneri:**
```toml
[[target]]
name = "header_only"
type = "interface"  # Sadece headers, no compilation
include_dirs = ["include"]
```

### 11. **Imported Targets**
**CMake'de:** `add_library(mylib SHARED IMPORTED)`
**ngmake'de:** ❌ Yok

**Sorun:** External pre-built library'ler için target tanımlanamıyor.

**Öneri:**
```toml
[[target]]
name = "external_lib"
type = "imported"
location = "/usr/lib/libexternal.so"
include_dirs = ["/usr/include/external"]
```

### 12. **Object Libraries**
**CMake'de:** `add_library(objlib OBJECT src1.cpp src2.cpp)`
**ngmake'de:** ❌ Yok

**Sorun:** Ortak object file'lar paylaşılamıyor.

**Öneri:**
```toml
[[target]]
name = "common_objects"
type = "object_lib"
sources = ["common.cpp"]
```

### 13. **Custom Commands / Custom Targets**
**CMake'de:** `add_custom_command()`, `add_custom_target()`
**ngmake'de:** ❌ Yok

**Sorun:** Pre-build, post-build script'leri yok.

**Öneri:**
```toml
[[target]]
name = "generate_code"
type = "custom"
command = "python generate.py"
deps = ["input.txt"]
outputs = ["generated.cpp"]
```

### 14. **RPATH Handling**
**CMake'de:** `set_target_properties(... PROPERTIES INSTALL_RPATH ...)`
**ngmake'de:** ⚠️ Kısmi (LD_LIBRARY_PATH var ama RPATH yok)

**Sorun:** Shared library'ler runtime'da bulunamayabilir.

**Öneri:**
```toml
[[target]]
name = "mylib"
rpath = "$ORIGIN"  # veya "$ORIGIN/../lib"
```

### 15. **Output Directory per Build Type**
**CMake'de:** `CMAKE_RUNTIME_OUTPUT_DIRECTORY_DEBUG`, `CMAKE_RUNTIME_OUTPUT_DIRECTORY_RELEASE`
**ngmake'de:** ❌ Tek output_dir var

**Sorun:** Debug ve Release build'leri aynı dizinde çakışır.

**Öneri:** Build type'a göre otomatik: `build/debug/`, `build/release/`

## 🟢 İyileştirme Önerileri

### 16. **Conditional Compilation**
**CMake'de:** `if()`, `option()`
**ngmake'de:** ❌ Yok

**Öneri:**
```toml
[project]
options = { 
    enable_tests = true,
    enable_benchmarks = false 
}

[[target]]
name = "test_math"
condition = "enable_tests"  # Sadece enable_tests=true ise build et
```

### 17. **Generator Expressions (Basit)**
**CMake'de:** `$<CONFIG:Debug>`, `$<TARGET_FILE:lib>`
**ngmake'de:** ❌ Yok

**Öneri:** Basit expression'lar:
```toml
[[target]]
flags = ["-O2", "$<IF:debug,-g,-DNDEBUG>"]
```

### 18. **Alias Targets**
**CMake'de:** `add_library(mylib::mylib ALIAS mylib)`
**ngmake'de:** ❌ Yok

**Öneri:**
```toml
[[target]]
name = "mylib::mylib"
type = "alias"
target = "mylib"
```

### 19. **Build Presets**
**CMake'de:** CMakePresets.json
**ngmake'de:** ❌ Yok

**Öneri:**
```toml
# build-presets.toml
[preset.debug]
build_type = "debug"
jobs = 1

[preset.release]
build_type = "release"
jobs = 8
```

### 20. **Package Management Integration**
**CMake'de:** vcpkg, Conan integration
**ngmake'de:** ❌ Yok

**Öneri:** vcpkg, Conan, veya basit package manager desteği.

### 21. **Workspace Root / Build Config Seçimi (GUI)**
**CMake'de:** Tek root’tan configure; IDE “current file” ile build’i değiştirmez.
**ngmake'de:** ⚠️ Build = seçilen build.toml; cross-module için her leaf’e `includes` eklenmeli.

**Öneri:** GUI’de “project root” tespiti: açık olan build.toml’u `includes` içinde geçiren üst build.toml varsa build’i onunla yap. Böylece tek modül dosyası açıkken bile tüm proje build edilir; "unknown dependency" kullanıcıya çıkmaz.

## 📊 Öncelik Sıralaması

### Yüksek Öncelik (Hemen eklenmeli)
1. ✅ Build Configurations (Debug/Release)
2. ✅ Preprocessor Definitions (ayrı field)
3. ✅ C++ Standard (ayrı field)
4. ✅ Linker Flags (ayrı field)
5. ✅ Compiler Detection

### Orta Öncelik (Yakın gelecekte)
6. Install Rules
7. Test Framework
8. Interface Libraries (tam destek)
9. RPATH Handling
10. Output Directory per Build Type

### Düşük Öncelik (Uzun vadede)
11. Cross-Compilation
12. Find Packages
13. Custom Commands/Targets
14. Object Libraries
15. Conditional Compilation

## 🎯 Sonuç

ngmake şu anda **basit projeler için yeterli** ama **enterprise-level projeler için eksikler var**. En kritik eksiklikler:

1. **Build configurations** - Debug/Release ayrımı yok
2. **Definitions/Standard** - flags içinde kayboluyor
3. **Install/Test** - Production-ready değil

Bu özellikler eklendiğinde ngmake, CMake'in basit alternatifi olarak çok daha güçlü olacak.


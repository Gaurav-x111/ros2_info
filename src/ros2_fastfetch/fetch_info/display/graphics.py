import os
import sys
import base64
import shutil
import subprocess
from rich.segment import Segment


def _normalize_ansi_fallback(text: str) -> str:
    """Convert generated literal escape sequences into real ANSI escapes."""
    if not text:
        return text
    return text.replace("\\x1b", "\x1b")


class BitmappedRenderable:
    """A Rich-compatible renderable that injects raw bitmapped sequences with text fallback."""
    def __init__(self, seq: str, width_cols: int, height_estimate: int, fallback_ansi: str = ""):
        self.seq = seq
        self.width_cols = width_cols
        self.height_estimate = height_estimate
        self.fallback = fallback_ansi

    def __rich_console__(self, console, options):
        # Emit bitmap protocol only when we are confident the terminal will draw it.
        if self.seq:
            yield Segment(self.seq)
            # Add newlines to reserve vertical space for the floating image
            yield Segment("\n" * self.height_estimate)
        elif self.fallback:
            from rich.text import Text
            yield Text.from_ansi(_normalize_ansi_fallback(self.fallback))

    def __rich_measure__(self, console, options):
        from rich.measure import Measurement
        return Measurement(self.width_cols, self.width_cols)


class KittenIcatRenderable:
    """Renderable that shells out to `kitten icat` for pixel-perfect image display.

    This bypasses Rich's rendering pipeline and writes directly to the
    terminal via subprocess, giving kitty full control over the image
    protocol (chunking, compression, placement).
    """

    def __init__(self, image_path: str, width_cols: int, fallback_ansi: str = ""):
        self.image_path = image_path
        self.width_cols = width_cols
        self.fallback = fallback_ansi
        # Estimate height in terminal rows (aspect ratio ≈ 2:1 for most cells)
        self.height_estimate = max(1, width_cols // 2)

    def __rich_console__(self, console, options):
        if self.image_path and os.path.exists(self.image_path):
            try:
                # Use kitten icat to render the image directly to the terminal.
                # --align left  : align to the left of the column
                # --place       : WxH@XxY placement in cells (handled by kitty)
                # We flush Rich's buffer first so the image lands in the right spot.
                fd = getattr(console.file, "fileno", lambda: None)()
                if fd is not None:
                    os.fsync(fd)

                cmd = [
                    "kitten", "icat",
                    "--align", "left",
                    "--place", f"{self.width_cols}x{self.height_estimate}@0x0",
                    self.image_path,
                ]
                subprocess.run(
                    cmd,
                    stdout=sys.stdout,
                    stderr=subprocess.DEVNULL,
                    timeout=5,
                )
                # Reserve vertical space so Rich doesn't overwrite the image
                yield Segment("\n" * self.height_estimate)
                return
            except Exception:
                pass  # fall through to fallback

        # Fallback to ANSI/Unicode art
        if self.fallback:
            from rich.text import Text
            yield Text.from_ansi(_normalize_ansi_fallback(self.fallback))

    def __rich_measure__(self, console, options):
        from rich.measure import Measurement
        return Measurement(self.width_cols, self.width_cols)


class ChafaRenderable:
    """Renderable that uses `chafa` to convert an image into terminal art.

    chafa auto-detects the best output mode for the current terminal
    (sixels, kitty protocol, iterm2, or plain symbols/half-blocks).
    This makes it the ideal **universal** renderer — it works in
    virtually any terminal emulator.
    """

    def __init__(self, image_path: str, width_cols: int, fallback_ansi: str = ""):
        self.image_path = image_path
        self.width_cols = width_cols
        self.fallback = fallback_ansi

    def __rich_console__(self, console, options):
        if self.image_path and os.path.exists(self.image_path):
            try:
                result = subprocess.run(
                    [
                        "chafa",
                        "--size", f"{self.width_cols}x",
                        "--animate=off",
                        self.image_path,
                    ],
                    capture_output=True,
                    text=True,
                    timeout=10,
                )
                if result.returncode == 0 and result.stdout.strip():
                    from rich.text import Text
                    yield Text.from_ansi(result.stdout)
                    return
            except Exception:
                pass  # fall through to fallback

        # Fallback to ANSI/Unicode art
        if self.fallback:
            from rich.text import Text
            yield Text.from_ansi(_normalize_ansi_fallback(self.fallback))

    def __rich_measure__(self, console, options):
        from rich.measure import Measurement
        return Measurement(self.width_cols, self.width_cols)


class GraphicsEngine:
    """Handles bitmapped terminal graphics using modern protocols."""

    # ── Kitty Graphics Protocol (with proper chunking) ──────────────────────
    @staticmethod
    def get_kitty_sequence(image_path: str, width_cols: int = 50) -> str:
        """Generates a Kitty Graphics Protocol sequence for true bitmap rendering.

        Properly chunks base64 data into ≤4096-byte segments as required by
        the protocol specification.
        """
        if not os.path.exists(image_path):
            return ""

        try:
            with open(image_path, "rb") as f:
                data = f.read()

            b64_data = base64.b64encode(data).decode("ascii")

            CHUNK_SIZE = 4096
            chunks = [b64_data[i : i + CHUNK_SIZE] for i in range(0, len(b64_data), CHUNK_SIZE)]

            if len(chunks) <= 1:
                # Small image — single transmission
                payload = chunks[0] if chunks else ""
                return f"\x1b_Ga=T,f=100,t=d,c={width_cols};{payload}\x1b\\"

            # Multi-chunk transmission
            parts = []
            for idx, chunk in enumerate(chunks):
                if idx == 0:
                    # First chunk: full header with m=1 (more data follows)
                    parts.append(f"\x1b_Ga=T,f=100,t=d,c={width_cols},m=1;{chunk}\x1b\\")
                elif idx == len(chunks) - 1:
                    # Last chunk: m=0 (no more data)
                    parts.append(f"\x1b_Gm=0;{chunk}\x1b\\")
                else:
                    # Middle chunks: m=1 (more data follows)
                    parts.append(f"\x1b_Gm=1;{chunk}\x1b\\")

            return "".join(parts)
        except Exception:
            return ""

    # ── iTerm2 Inline Image Protocol ────────────────────────────────────────
    @staticmethod
    def get_iterm2_sequence(image_path: str, width_cols: int = 50) -> str:
        """Generates an iTerm2 Inline Image Protocol sequence (Standard VSCode variant)."""
        if not os.path.exists(image_path):
            return ""

        try:
            with open(image_path, "rb") as f:
                data = f.read()

            b64_data = base64.b64encode(data).decode("ascii")
            # Width without a unit is interpreted in terminal cells.
            return (
                f"\x1b]1337;File=inline=1;width={width_cols};height=auto;"
                f"preserveAspectRatio=1:{b64_data}\x07"
            )
        except Exception:
            return ""

    # ── Terminal Detection ──────────────────────────────────────────────────
    @staticmethod
    def _detect_terminal() -> str:
        """Detect the terminal emulator in use.

        Returns one of: 'kitty', 'iterm', 'wezterm', 'vscode', 'unknown'.
        """
        term_program = os.environ.get("TERM_PROGRAM", "").lower()

        # Kitty sets KITTY_WINDOW_ID and TERM=xterm-kitty
        if os.environ.get("KITTY_WINDOW_ID"):
            return "kitty"
        if "kitty" in os.environ.get("TERM", ""):
            return "kitty"
        if "kitty" in term_program:
            return "kitty"

        if term_program == "vscode":
            return "vscode"
        if "iterm" in term_program:
            return "iterm"
        if "wezterm" in term_program:
            return "wezterm"

        return "unknown"

    @staticmethod
    def _kitten_available() -> bool:
        """Check whether the `kitten` binary is on PATH."""
        return shutil.which("kitten") is not None

    @staticmethod
    def _chafa_available() -> bool:
        """Check whether the `chafa` binary is on PATH."""
        return shutil.which("chafa") is not None

    # ── Main Render Dispatch ────────────────────────────────────────────────
    @classmethod
    def render(cls, image_path: str, fallback_ansi: str, width_cols: int = 50):
        """Selects the best protocol and returns a renderable with fallback.

        Priority order:
          1. kitten icat  (kitty terminals — pixel-perfect)
          2. chafa        (universal — works in ANY terminal, auto-detects best mode)
          3. Kitty Graphics Protocol (WezTerm / kitty without kitten)
          4. iTerm2 Protocol (real iTerm2 only)
          5. Hardcoded ANSI / Unicode fallback art
        """
        terminal = cls._detect_terminal()

        # ── Kitty-native: prefer kitten icat (pixel-perfect) ────────────────
        if terminal == "kitty" and cls._kitten_available() and image_path:
            return KittenIcatRenderable(image_path, width_cols, fallback_ansi)

        # ── chafa: universal, works in ANY terminal ─────────────────────────
        # Preferred for VSCode, unknown terminals, and everything else.
        # chafa auto-detects sixels, kitty protocol, iterm2, or symbols.
        if cls._chafa_available() and image_path and os.path.exists(image_path):
            return ChafaRenderable(image_path, width_cols, fallback_ansi)

        seq = ""

        # iTerm2 protocol — only for real iTerm2 (not VSCode, it's unreliable)
        if terminal == "iterm":
            seq = cls.get_iterm2_sequence(image_path, width_cols)

        # Kitty protocol (manual) for WezTerm or kitty without kitten binary
        if not seq and terminal in ("wezterm", "kitty"):
            seq = cls.get_kitty_sequence(image_path, width_cols)

        if seq:
            height_estimate = width_cols // 2
            return BitmappedRenderable(seq, width_cols, height_estimate, fallback_ansi)

        # ── Ultimate fallback: hardcoded ANSI art ───────────────────────────
        return BitmappedRenderable("", width_cols, 0, fallback_ansi)

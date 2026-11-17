import type { CreateServer, Loader, Property } from "../../api/client";
import type { ContentItem } from "../../api/content";
import { memoryFor } from "../../memory";

export const STANDARD_LOADERS: readonly Loader[] = ["paper", "purpur", "vanilla"];
/** Server hỗ trợ cài plugin ngay khi tạo. */
export const PLUGIN_LOADERS: readonly Loader[] = ["paper", "purpur"];
/** Thứ tự theo spec P3 mục 6.1 và 11.7. */
export const MOD_LOADERS: readonly Loader[] = ["fabric", "neoforge", "forge", "quilt"];

export function isModLoader(loader: Loader): boolean {
  return (MOD_LOADERS as readonly string[]).includes(loader);
}

export function isPluginLoader(loader: Loader): boolean {
  return (PLUGIN_LOADERS as readonly string[]).includes(loader);
}

export function contentKindForLoader(loader: Loader): "mod" | "plugin" | null {
  if (isModLoader(loader)) return "mod";
  if (isPluginLoader(loader)) return "plugin";
  return null;
}

export interface GameSettingsValues {
  gamemode: string;
  difficulty: string;
  maxPlayers: string;
  motd: string;
  onlineMode: boolean;
  whiteList: boolean;
  pvp: boolean;
  viewDistance: string;
  spawnProtection: string;
  levelSeed: string;
  enableCommandBlocks: boolean;
  allowFlight: boolean;
}

export const DEFAULT_GAME_SETTINGS: GameSettingsValues = {
  gamemode: "survival",
  difficulty: "easy",
  maxPlayers: "20",
  motd: "",
  onlineMode: true,
  whiteList: false,
  pvp: true,
  viewDistance: "10",
  spawnProtection: "16",
  levelSeed: "",
  enableCommandBlocks: false,
  allowFlight: false,
};

export interface WizardForm {
  loader: Loader;
  /** Rỗng: chưa chọn, dùng bản mới nhất. */
  version: string;
  /** Rỗng hoặc undefined: dùng bản đề xuất đầu danh sách. */
  loaderVersion?: string;
  name: string;
  /** `null`: dùng mức đề xuất của backend. */
  memoryMb: number | null;
  /** Rỗng: để backend tự chọn cổng trống. */
  port: string;
  game?: GameSettingsValues;
}

const NAME_MAX_CHARS = 64;
const PORT_MIN = 1024;
const PORT_MAX = 65535;

function normalizeInt(raw: string): string {
  const trimmed = raw.trim();
  return /^\d+$/.test(trimmed) ? String(Number(trimmed)) : trimmed;
}

function intRangeError(raw: string, min: number, max: number, key: string): string | undefined {
  const trimmed = raw.trim();
  if (trimmed === "") return undefined;
  if (!/^\d+$/.test(trimmed)) return key;
  const value = Number(trimmed);
  return Number.isSafeInteger(value) && value >= min && value <= max ? undefined : key;
}

/** Khóa i18n của lỗi, hoặc `undefined` khi hợp lệ. Đếm theo ký tự như backend, không theo UTF-16. */
export function nameError(name: string): string | undefined {
  const length = [...name.trim()].length;
  return length === 0 || length > NAME_MAX_CHARS ? "wizard.nameInvalid" : undefined;
}

export function portError(port: string): string | undefined {
  return intRangeError(port, PORT_MIN, PORT_MAX, "wizard.portInvalid");
}

export function maxPlayersError(value: string): string | undefined {
  return intRangeError(value, 1, 100_000, "wizard.maxPlayersInvalid");
}

export function viewDistanceError(value: string): string | undefined {
  return intRangeError(value, 2, 32, "wizard.viewDistanceInvalid");
}

export function spawnProtectionError(value: string): string | undefined {
  return intRangeError(value, 0, 1000, "wizard.spawnProtectionInvalid");
}

export function hasGameSettingsError(game?: GameSettingsValues): boolean {
  if (!game) return false;
  return Boolean(
    maxPlayersError(game.maxPlayers) ||
      viewDistanceError(game.viewDistance) ||
      spawnProtectionError(game.spawnProtection),
  );
}

/** Cho biết có trường nào trong mục Nâng cao khác mặc định (để giữ mở khi quay lại bước trước). */
export function hasAdvancedChanges(port: string, game?: GameSettingsValues): boolean {
  if (port.trim() !== "") return true;
  if (!game) return false;
  return (
    normalizeInt(game.viewDistance) !== DEFAULT_GAME_SETTINGS.viewDistance ||
    normalizeInt(game.spawnProtection) !== DEFAULT_GAME_SETTINGS.spawnProtection ||
    game.levelSeed.trim() !== "" ||
    game.enableCommandBlocks !== DEFAULT_GAME_SETTINGS.enableCommandBlocks ||
    game.allowFlight !== DEFAULT_GAME_SETTINGS.allowFlight
  );
}

/** Chỉ trả các khóa khác mặc định để không ghi đè `server.properties` sẵn có của modpack. */
export function toInitialProperties(game?: GameSettingsValues): Property[] {
  if (!game) return [];
  const props: Property[] = [];
  if (game.gamemode !== DEFAULT_GAME_SETTINGS.gamemode) {
    props.push({ key: "gamemode", value: game.gamemode });
  }
  if (game.difficulty !== DEFAULT_GAME_SETTINGS.difficulty) {
    props.push({ key: "difficulty", value: game.difficulty });
  }
  const maxPlayers = normalizeInt(game.maxPlayers);
  if (maxPlayers !== "" && maxPlayers !== DEFAULT_GAME_SETTINGS.maxPlayers) {
    props.push({ key: "max-players", value: maxPlayers });
  }
  const motd = game.motd.trim();
  if (motd !== "") {
    props.push({ key: "motd", value: motd });
  }
  if (game.onlineMode !== DEFAULT_GAME_SETTINGS.onlineMode) {
    props.push({ key: "online-mode", value: String(game.onlineMode) });
  }
  if (game.whiteList !== DEFAULT_GAME_SETTINGS.whiteList) {
    props.push({ key: "white-list", value: String(game.whiteList) });
  }
  if (game.pvp !== DEFAULT_GAME_SETTINGS.pvp) {
    props.push({ key: "pvp", value: String(game.pvp) });
  }
  const viewDistance = normalizeInt(game.viewDistance);
  if (viewDistance !== "" && viewDistance !== DEFAULT_GAME_SETTINGS.viewDistance) {
    props.push({ key: "view-distance", value: viewDistance });
  }
  const spawnProtection = normalizeInt(game.spawnProtection);
  if (spawnProtection !== "" && spawnProtection !== DEFAULT_GAME_SETTINGS.spawnProtection) {
    props.push({ key: "spawn-protection", value: spawnProtection });
  }
  const levelSeed = game.levelSeed.trim();
  if (levelSeed !== "") {
    props.push({ key: "level-seed", value: levelSeed });
  }
  if (game.enableCommandBlocks !== DEFAULT_GAME_SETTINGS.enableCommandBlocks) {
    props.push({ key: "enable-command-blocks", value: String(game.enableCommandBlocks) });
  }
  if (game.allowFlight !== DEFAULT_GAME_SETTINGS.allowFlight) {
    props.push({ key: "allow-flight", value: String(game.allowFlight) });
  }
  return props;
}

/** `content`: mod/plugin chọn ở bước Mod/Plugin, cài ngay sau khi tạo server. */
export function toRequest(form: WizardForm, version: string, content: ContentItem[] = []): CreateServer {
  const req: CreateServer = {
    name: form.name.trim(),
    loader: form.loader,
    minecraft: version,
    memory_mb: form.memoryMb === null ? null : memoryFor(form.memoryMb),
    port: form.port.trim() === "" ? null : Number(form.port),
  };
  if (isModLoader(form.loader) && form.loaderVersion) {
    req.loader_version = form.loaderVersion;
  }
  if (content.length > 0) req.content = content;
  const properties = toInitialProperties(form.game);
  if (properties.length > 0) req.properties = properties;
  return req;
}

/**
 * Utilitaires discord.js / discordx du bot : intents accordés, cache borné,
 * exécution résiliente des interactions, enregistrement des commandes.
 */
import type { Client } from "discordx";
import {
  type ChatInputCommandInteraction,
  GatewayIntentBits,
  type Interaction,
  MessageFlags,
  Options,
} from "discord.js";

type Logger = Partial<
  Pick<Console, "info" | "warn" | "error">
>;

const FULL_INTENTS: GatewayIntentBits[] = [
  GatewayIntentBits.Guilds,
  GatewayIntentBits.GuildMembers,
  GatewayIntentBits.GuildMessages,
  GatewayIntentBits.GuildMessageReactions,
  GatewayIntentBits.GuildVoiceStates,
  GatewayIntentBits.GuildPresences,
  GatewayIntentBits.DirectMessages,
  GatewayIntentBits.MessageContent,
];

const PRIVILEGED = new Set<GatewayIntentBits>([
  GatewayIntentBits.GuildMembers,
  GatewayIntentBits.GuildPresences,
  GatewayIntentBits.MessageContent,
]);

/** Bits de `application.flags` qui confirment l'approbation de chaque intent privilégié. */
const GRANT_BITS = new Map<GatewayIntentBits, number[]>([
  [GatewayIntentBits.GuildPresences, [4096, 8192]],
  [GatewayIntentBits.GuildMembers, [16384, 32768]],
  [GatewayIntentBits.MessageContent, [1 << 18, 1 << 19]],
]);

const withoutPrivileged = (intents: readonly GatewayIntentBits[]) =>
  intents.filter((intent) => !PRIVILEGED.has(intent));

export interface ResolveIntentsOptions {
  desiredIntents?: readonly GatewayIntentBits[];
  timeoutMs?: number;
  apiBase?: string;
  fetchImpl?: typeof fetch;
  logger?: Logger;
}

/**
 * Interroge `GET /applications/@me` avant la construction du client et retire les
 * intents privilégiés non activés dans le portail développeur (sinon la passerelle
 * ferme avec le code 4014).
 */
export async function resolveGrantedIntents(
  botToken: string,
  options: ResolveIntentsOptions = {},
): Promise<GatewayIntentBits[]> {
  const desired = options.desiredIntents ?? FULL_INTENTS;
  const fetchFn = options.fetchImpl ?? globalThis.fetch;
  const logger = options.logger ?? console;
  const apiBase = options.apiBase ?? "https://discord.com/api/v10";
  let flags = 0;
  try {
    const res = await fetchFn(`${apiBase}/applications/@me`, {
      headers: { Authorization: `Bot ${botToken}` },
      signal: AbortSignal.timeout(options.timeoutMs ?? 10_000),
    });
    if (!res.ok) {
      logger.warn?.(
        `[discord] Pré-vol GET /applications/@me : HTTP ${res.status}. Repli sur les intents non privilégiés (évite la 4014).`,
      );
      return withoutPrivileged(desired);
    }
    flags = ((await res.json()) as { flags?: number }).flags ?? 0;
  } catch (err) {
    logger.warn?.(
      `[discord] Pré-vol des intents impossible : ${err instanceof Error ? err.message : String(err)}. Repli sur les intents non privilégiés.`,
    );
    return withoutPrivileged(desired);
  }
  const refused: string[] = [];
  const granted = desired.filter((intent) => {
    const bits = GRANT_BITS.get(intent);
    if (!bits || bits.some((b) => (flags & b) !== 0)) return true;
    refused.push(GatewayIntentBits[intent] ?? String(intent));
    return false;
  });
  if (refused.length > 0) {
    (logger.warn ?? console.warn)(
      `[discord] Intents privilégiés non accordés dans le portail développeur : ${refused.join(", ")}. Mode dégradé sans eux (évite la 4014). Activation : Discord Dev Portal > Applications > Bot > Privileged Gateway Intents.`,
    );
  }
  return [...granted];
}

export interface CacheConfig {
  messageLimit?: number;
  threadLimit?: number;
  reactionLimit?: number;
  stageInstanceLimit?: number;
  messageSweeperInterval?: number;
  messageSweeperLifetime?: number;
  threadSweeperInterval?: number;
  threadSweeperLifetime?: number;
  sweeperOverrides?: Record<string, unknown>;
}

/** Caches bornés et balayeurs pour un client 24/7 (sans eux discord.js garde tout en RAM). */
export function createProductionCacheOptions(config: CacheConfig = {}) {
  return {
    makeCache: Options.cacheWithLimits({
      MessageManager: config.messageLimit ?? 200,
      ThreadManager: config.threadLimit ?? 50,
      ReactionManager: config.reactionLimit ?? 0,
      StageInstanceManager: config.stageInstanceLimit ?? 0,
    }),
    sweepers: {
      ...Options.DefaultSweeperSettings,
      messages: {
        interval: config.messageSweeperInterval ?? 3600,
        lifetime: config.messageSweeperLifetime ?? 1800,
      },
      threads: {
        interval: config.threadSweeperInterval ?? 3600,
        lifetime: config.threadSweeperLifetime ?? 3600,
      },
      ...config.sweeperOverrides,
    },
  };
}

export interface SafeExecuteOptions {
  logger?: Logger;
  failureMessage?: string;
  unhandledMessage?: string;
}

/**
 * Exécute une interaction sans jamais planter le service : erreur journalisée,
 * autocomplétion ignorée, `deferReply` orphelin récupéré par `editReply`,
 * jeton expiré (10062) ignoré.
 */
export async function safeExecuteInteraction(
  client: Client,
  interaction: Interaction | ChatInputCommandInteraction,
  options: SafeExecuteOptions = {},
): Promise<void> {
  const logger = options.logger ?? console;
  const failureMessage =
    options.failureMessage ?? "❌ La commande a rencontré une erreur interne.";
  const unhandledMessage =
    options.unhandledMessage ??
    "Commande inconnue ou expirée. Rechargez Discord (Ctrl+R) pour actualiser la liste.";
  let failed = false;
  try {
    await client.executeInteraction(interaction);
  } catch (err) {
    failed = true;
    logger.error?.(
      "[discord] Exécution d'interaction en échec :",
      err instanceof Error ? (err.stack ?? err.message) : err,
    );
  }
  if (!interaction.isRepliable()) return;
  try {
    if (interaction.replied) return;
    if (interaction.deferred) {
      if (failed) await interaction.editReply({ content: failureMessage });
      return;
    }
    await interaction.reply({
      content: failed ? failureMessage : unhandledMessage,
      flags: MessageFlags.Ephemeral,
    });
  } catch (replyErr) {
    if ((replyErr as { code?: number })?.code === 10062) return;
    logger.warn?.(
      "[discord] Réponse de repli impossible :",
      replyErr instanceof Error ? (replyErr.stack ?? replyErr.message) : replyErr,
    );
  }
}

/** Enregistre les commandes slash ; journalise le chemin `rawError` en cas de « Invalid Form Body ». */
export async function initApplicationCommandsSafe(
  client: Client,
  options: { logger?: Logger } = {},
): Promise<boolean> {
  const logger = options.logger ?? console;
  try {
    await client.initApplicationCommands();
    logger.info?.("[discord] Commandes applicatives initialisées");
    return true;
  } catch (err) {
    const raw = (err as { rawError?: unknown }).rawError;
    logger.error?.(
      "[discord] initApplicationCommands en échec :",
      err instanceof Error ? err.message : String(err),
      raw ? `\n  rawError Discord : ${JSON.stringify(raw)}` : "",
    );
    return false;
  }
}

/** Efface les commandes de guilde sur les seules guildes rejointes ; renvoie leur nombre. */
export async function clearGuildCommandsSafe(
  client: Client,
  guildIds: readonly string[],
  options: { logger?: Logger } = {},
): Promise<number> {
  const logger = options.logger ?? console;
  const joined = guildIds.filter((id) => client.guilds.cache.has(id));
  if (joined.length === 0) return 0;
  try {
    await client.clearApplicationCommands(...joined);
    logger.info?.(
      `[discord] Commandes de guilde effacées sur ${joined.length} guilde(s)`,
    );
    return joined.length;
  } catch (err) {
    logger.warn?.(
      "[discord] Effacement des commandes de guilde impossible (doublons possibles) :",
      err instanceof Error ? err.message : String(err),
    );
    return 0;
  }
}

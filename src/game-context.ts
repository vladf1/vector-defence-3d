import { getContext, setContext } from "svelte";
import type { GameSession } from "./game-session";

const GAME_SESSION_KEY = Symbol("game-session");

export function setGameSessionContext(session: GameSession): GameSession {
  setContext(GAME_SESSION_KEY, session);
  return session;
}

export function getGameSessionContext(): GameSession {
  const session = getContext<GameSession | undefined>(GAME_SESSION_KEY);
  if (!session) {
    throw new Error("Missing game session context.");
  }

  return session;
}

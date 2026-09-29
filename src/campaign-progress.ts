import { clamp } from "./utils";

const HIGHEST_UNLOCKED_LEVEL_STORAGE_KEY = "vector-defence-2026:highest-unlocked-level:v1";
const CAMPAIGN_CLEARED_STORAGE_KEY = "vector-defence-2026:campaign-cleared:v1";
const LEVEL_STARS_STORAGE_KEY_PREFIX = "vector-defence-2026:level-stars:v1:";

export interface CampaignProgress {
  highestUnlockedLevelIndex: number;
  campaignCleared: boolean;
}

export class CampaignProgressStore {
  private readonly memory = new Map<string, string>();
  private storage?: Storage;

  constructor(storage: Storage | undefined) {
    this.storage = storage;
  }

  loadCampaignProgress(levelCount: number): CampaignProgress {
    const savedLevelIndex = this.read(HIGHEST_UNLOCKED_LEVEL_STORAGE_KEY);
    const campaignCleared = this.read(CAMPAIGN_CLEARED_STORAGE_KEY) === "true";
    const highestUnlockedLevelIndex = Number(savedLevelIndex ?? 0);
    const finalCampaignLevelIndex = Math.max(levelCount - 1, 0);

    return {
      campaignCleared,
      highestUnlockedLevelIndex: campaignCleared
        ? finalCampaignLevelIndex
        : (Number.isInteger(highestUnlockedLevelIndex)
            ? clamp(highestUnlockedLevelIndex, 0, finalCampaignLevelIndex)
            : 0),
    };
  }

  loadLevelStars(levelCount: number): number[] {
    return Array.from({ length: levelCount }, (_, index) => {
      const savedStars = Number(this.read(`${LEVEL_STARS_STORAGE_KEY_PREFIX}${index}`) ?? 0);
      return Number.isInteger(savedStars) ? clamp(savedStars, 0, 3) : 0;
    });
  }

  saveCampaignProgress(highestUnlockedLevelIndex: number, campaignCleared: boolean): void {
    this.write(HIGHEST_UNLOCKED_LEVEL_STORAGE_KEY, String(highestUnlockedLevelIndex));
    if (campaignCleared) {
      this.write(CAMPAIGN_CLEARED_STORAGE_KEY, "true");
    } else {
      this.remove(CAMPAIGN_CLEARED_STORAGE_KEY);
    }
  }

  saveLevelStars(levelIndex: number, stars: number): void {
    this.write(`${LEVEL_STARS_STORAGE_KEY_PREFIX}${levelIndex}`, String(stars));
  }

  private read(key: string): string | null {
    const memoryValue = this.memory.get(key) ?? null;
    if (!this.storage) {
      return memoryValue;
    }

    try {
      const value = this.storage.getItem(key);
      if (value === null) {
        this.memory.delete(key);
      } else {
        this.memory.set(key, value);
      }
      return value;
    } catch {
      this.storage = undefined;
      return memoryValue;
    }
  }

  private write(key: string, value: string): void {
    this.memory.set(key, value);
    if (!this.storage) {
      return;
    }

    try {
      this.storage.setItem(key, value);
    } catch {
      this.storage = undefined;
    }
  }

  private remove(key: string): void {
    this.memory.delete(key);
    if (!this.storage) {
      return;
    }

    try {
      this.storage.removeItem(key);
    } catch {
      this.storage = undefined;
    }
  }
}

export function createBrowserCampaignProgressStore(target: Window): CampaignProgressStore {
  try {
    return new CampaignProgressStore(target.localStorage);
  } catch {
    return new CampaignProgressStore(undefined);
  }
}

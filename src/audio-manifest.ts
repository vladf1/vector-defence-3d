import campaignCompleteUrl from "./assets/audio/campaign-complete.m4a";
import escapeBurstUrl from "./assets/audio/escape-burst.m4a";
import gunFireUrl from "./assets/audio/gun-fire.m4a";
import invalidActionUrl from "./assets/audio/invalid-action.m4a";
import laserFireUrl from "./assets/audio/laser-fire.m4a";
import laserLockOffUrl from "./assets/audio/laser-lock-off.m4a";
import laserLockOnUrl from "./assets/audio/laser-lock-on.m4a";
import lightningShockUrl from "./assets/audio/lightning-shock.m4a";
import levelLossUrl from "./assets/audio/level-loss.m4a";
import levelStartUrl from "./assets/audio/level-start.m4a";
import levelWinUrl from "./assets/audio/level-win.m4a";
import menuOpenUrl from "./assets/audio/menu-open.m4a";
import missileExplosionUrl from "./assets/audio/missile-explosion.m4a";
import missileLaunchUrl from "./assets/audio/missile-launch.m4a";
import monsterHeavyDeathUrl from "./assets/audio/monster-heavy-death.m4a";
import monsterPopUrl from "./assets/audio/monster-pop.m4a";
import monsterShatterUrl from "./assets/audio/monster-shatter.m4a";
import pauseUrl from "./assets/audio/pause.m4a";
import projectileImpactUrl from "./assets/audio/projectile-impact.m4a";
import resumeUrl from "./assets/audio/resume.m4a";
import slowPulseUrl from "./assets/audio/slow-pulse.m4a";
import soundToggleUrl from "./assets/audio/sound-toggle.m4a";
import splitterBurstUrl from "./assets/audio/splitter-burst.m4a";
import towerPlaceUrl from "./assets/audio/tower-place.m4a";
import towerSelectUrl from "./assets/audio/tower-select.m4a";
import towerSellUrl from "./assets/audio/tower-sell.m4a";
import towerUpgradeUrl from "./assets/audio/tower-upgrade.m4a";
import uiClickUrl from "./assets/audio/ui-click.m4a";
import uiConfirmUrl from "./assets/audio/ui-confirm.m4a";
import waveClearUrl from "./assets/audio/wave-clear.m4a";
import waveStartUrl from "./assets/audio/wave-start.m4a";

interface AudioCueDefinition {
  readonly id: string;
  readonly label: string;
  readonly url: string;
  readonly cooldownSeconds: number;
  readonly gain: number;
  readonly rateVariation: number;
}

export const AudioCue = {
  CampaignComplete: { id: "campaign-complete", label: "Campaign Complete", url: campaignCompleteUrl, cooldownSeconds: 0.8, gain: 0.48, rateVariation: 0 },
  EscapeBurst: { id: "escape-burst", label: "Escape Burst", url: escapeBurstUrl, cooldownSeconds: 0.16, gain: 0.52, rateVariation: 0 },
  GunFire: { id: "gun-fire", label: "Gun Fire", url: gunFireUrl, cooldownSeconds: 0.026, gain: 0.58, rateVariation: 0.035 },
  InvalidAction: { id: "invalid-action", label: "Invalid Action", url: invalidActionUrl, cooldownSeconds: 0.12, gain: 0.32, rateVariation: 0 },
  LaserFire: { id: "laser-fire", label: "Laser Fire", url: laserFireUrl, cooldownSeconds: 0.18, gain: 0.45, rateVariation: 0 },
  LaserLockOff: { id: "laser-lock-off", label: "Laser Lock Off", url: laserLockOffUrl, cooldownSeconds: 0.08, gain: 0.34, rateVariation: 0 },
  LaserLockOn: { id: "laser-lock-on", label: "Laser Lock On", url: laserLockOnUrl, cooldownSeconds: 0.08, gain: 0.34, rateVariation: 0 },
  LightningShock: { id: "lightning-shock", label: "Lightning Shock", url: lightningShockUrl, cooldownSeconds: 0.09, gain: 0.34, rateVariation: 0.035 },
  LevelLoss: { id: "level-loss", label: "Level Loss", url: levelLossUrl, cooldownSeconds: 0.8, gain: 0.46, rateVariation: 0 },
  LevelStart: { id: "level-start", label: "Level Start", url: levelStartUrl, cooldownSeconds: 0.25, gain: 0.36, rateVariation: 0 },
  LevelWin: { id: "level-win", label: "Level Win", url: levelWinUrl, cooldownSeconds: 0.8, gain: 0.42, rateVariation: 0 },
  MenuOpen: { id: "menu-open", label: "Menu Open", url: menuOpenUrl, cooldownSeconds: 0.12, gain: 0.32, rateVariation: 0 },
  MissileExplosion: { id: "missile-explosion", label: "Missile Explosion", url: missileExplosionUrl, cooldownSeconds: 0.08, gain: 0.4, rateVariation: 0 },
  MissileLaunch: { id: "missile-launch", label: "Missile Launch", url: missileLaunchUrl, cooldownSeconds: 0.08, gain: 0.54, rateVariation: 0.025 },
  MonsterHeavyDeath: { id: "monster-heavy-death", label: "Monster Heavy Death", url: monsterHeavyDeathUrl, cooldownSeconds: 0.08, gain: 0.7, rateVariation: 0 },
  MonsterPop: { id: "monster-pop", label: "Monster Pop", url: monsterPopUrl, cooldownSeconds: 0.035, gain: 0.44, rateVariation: 0.06 },
  MonsterShatter: { id: "monster-shatter", label: "Monster Shatter", url: monsterShatterUrl, cooldownSeconds: 0.045, gain: 0.42, rateVariation: 0.04 },
  Pause: { id: "pause", label: "Pause", url: pauseUrl, cooldownSeconds: 0.08, gain: 0.32, rateVariation: 0 },
  ProjectileImpact: { id: "projectile-impact", label: "Projectile Impact", url: projectileImpactUrl, cooldownSeconds: 0.018, gain: 0.34, rateVariation: 0.06 },
  Resume: { id: "resume", label: "Resume", url: resumeUrl, cooldownSeconds: 0.08, gain: 0.32, rateVariation: 0 },
  SlowPulse: { id: "slow-pulse", label: "Slow Pulse", url: slowPulseUrl, cooldownSeconds: 0.11, gain: 0.34, rateVariation: 0 },
  SoundToggle: { id: "sound-toggle", label: "Sound Toggle", url: soundToggleUrl, cooldownSeconds: 0.08, gain: 0.32, rateVariation: 0 },
  SplitterBurst: { id: "splitter-burst", label: "Splitter Burst", url: splitterBurstUrl, cooldownSeconds: 0.1, gain: 0.5, rateVariation: 0 },
  TowerPlace: { id: "tower-place", label: "Tower Place", url: towerPlaceUrl, cooldownSeconds: 0.05, gain: 0.44, rateVariation: 0 },
  TowerSelect: { id: "tower-select", label: "Tower Select", url: towerSelectUrl, cooldownSeconds: 0.06, gain: 0.28, rateVariation: 0 },
  TowerSell: { id: "tower-sell", label: "Tower Sell", url: towerSellUrl, cooldownSeconds: 0.08, gain: 0.34, rateVariation: 0 },
  TowerUpgrade: { id: "tower-upgrade", label: "Tower Upgrade", url: towerUpgradeUrl, cooldownSeconds: 0.08, gain: 0.38, rateVariation: 0 },
  UiClick: { id: "ui-click", label: "UI Click", url: uiClickUrl, cooldownSeconds: 0.04, gain: 0.28, rateVariation: 0 },
  UiConfirm: { id: "ui-confirm", label: "UI Confirm", url: uiConfirmUrl, cooldownSeconds: 0.08, gain: 0.34, rateVariation: 0 },
  WaveClear: { id: "wave-clear", label: "Wave Clear", url: waveClearUrl, cooldownSeconds: 0.5, gain: 0.34, rateVariation: 0 },
  WaveStart: { id: "wave-start", label: "Wave Start", url: waveStartUrl, cooldownSeconds: 0.4, gain: 0.34, rateVariation: 0 },
} as const satisfies Record<string, AudioCueDefinition>;

export type AudioCue = typeof AudioCue[keyof typeof AudioCue];

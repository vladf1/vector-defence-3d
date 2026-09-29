//! Sound cues the simulation asks the page to play. The discriminants index
//! `AUDIO_CUE_ORDER` in `src/audio-manifest.ts`; keep both lists in the same order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AudioCue {
    CampaignComplete,
    EscapeBurst,
    GunFire,
    InvalidAction,
    LaserFire,
    LaserLockOff,
    LaserLockOn,
    LightningShock,
    LevelLoss,
    LevelStart,
    LevelWin,
    MenuOpen,
    MissileExplosion,
    MissileLaunch,
    MonsterHeavyDeath,
    MonsterPop,
    MonsterShatter,
    Pause,
    ProjectileImpact,
    Resume,
    SlowPulse,
    SoundToggle,
    SplitterBurst,
    TowerPlace,
    TowerSelect,
    TowerSell,
    TowerUpgrade,
    UiClick,
    UiConfirm,
    WaveClear,
    WaveStart,
}

/// One requested playback; `None` leaves the manifest's default pan or intensity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundEvent {
    pub cue: AudioCue,
    pub pan_x: Option<f64>,
    pub intensity: Option<f64>,
}

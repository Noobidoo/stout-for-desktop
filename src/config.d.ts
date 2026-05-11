declare type DesktopConfig = {
  firstLaunch: boolean;
  customFrame: boolean;
  minimiseToTray: boolean;
  startMinimisedToTray: boolean;
  spellchecker: boolean;
  hardwareAcceleration: boolean;
  discordRpc: boolean;
  pttKey: string | null;
  windowState: {
    x: number;
    y: number;
    width: number;
    height: number;
    isMaximised: boolean;
  };
};

declare interface Window {
  native: {
    versions: {
      node: () => string;
      chrome: () => string;
      electron: () => string;
      desktop: () => string;
    };
    minimise: () => void;
    maximise: () => void;
    close: () => void;
    setBadgeCount: (count: number) => void;
    ptt: {
      /** Returns an unlisten function */
      onPress: (cb: () => void) => Promise<() => void>;
      /** Returns an unlisten function */
      onRelease: (cb: () => void) => Promise<() => void>;
    };
  };
  desktopConfig: {
    get: () => DesktopConfig;
    set: (config: DesktopConfig) => Promise<DesktopConfig>;
    getAutostart: () => Promise<boolean>;
    setAutostart: (value: boolean) => Promise<boolean>;
    /** Key string e.g. "F13", "Space", "Alt+G". Persisted to disk. */
    registerPttKey: (key: string) => Promise<void>;
    /** Clears PTT key and persists. */
    unregisterPttKey: () => Promise<void>;
  };
}


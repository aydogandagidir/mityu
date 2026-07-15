import { invoke } from '@tauri-apps/api/core';

export interface AnalyticsProperties {
  [key: string]: string;
}

export interface DeviceInfo {
  platform: string;
  os_version: string;
  architecture: string;
}

export interface UserSession {
  session_id: string;
  user_id: string;
  start_time: string;
  last_heartbeat: string;
  is_active: boolean;
}

const PROFILE_PROPERTY_KEYS = new Set(['platform', 'architecture']);

function isValidInstallationId(value: string): boolean {
  return value.length <= 96 && /^user_[A-Za-z0-9_-]{6,}$/.test(value);
}

const EVENT_PROPERTY_ALLOWLIST: Readonly<Record<string, readonly string[]>> = {
  analytics_disabled: [],
  analytics_enabled: [],
  analytics_transparency_viewed: [],
  app_started: ['app_version'],
  auto_save_recording_toggled: ['enabled'],
  backend_connection: ['success'],
  beta_feature_toggled: ['feature', 'enabled'],
  button_click: ['button', 'location'],
  custom_prompt_used: ['prompt_length'],
  daily_active_user: [],
  default_devices_changed: ['has_preferred_microphone', 'has_preferred_system_audio'],
  enhance_transcript_completed: ['success', 'duration_seconds', 'segments_count'],
  enhance_transcript_started: ['language', 'model_provider', 'model_name'],
  error: ['error_type'],
  feature_used: ['feature_name', 'is_first_use', 'platform', 'architecture'],
  import_audio_completed: ['success', 'duration_seconds', 'segments_count'],
  import_audio_started: ['file_size_bytes', 'duration_seconds', 'language', 'model_provider', 'model_name'],
  language_selected: ['language_code', 'is_auto_detect', 'is_auto_translate'],
  meeting_completed: [
    'duration_seconds',
    'transcript_segments',
    'transcript_word_count',
    'words_per_minute',
    'meetings_today',
    'day_of_week',
    'hour_of_day',
    'platform',
    'architecture',
  ],
  meeting_deleted: [],
  meeting_ended: [
    'transcription_provider',
    'transcription_model',
    'summary_provider',
    'summary_model',
    'total_duration_seconds',
    'active_duration_seconds',
    'pause_duration_seconds',
    'microphone_device_type',
    'system_audio_device_type',
    'chunks_processed',
    'transcript_segments_count',
    'had_fatal_error',
  ],
  meeting_started: [],
  microphone_selected: ['device_category', 'is_bluetooth', 'has_system_audio'],
  model_changed: ['old_provider', 'old_model', 'new_provider', 'new_model'],
  notification_settings_changed: ['notifications_enabled'],
  page_view: ['page'],
  preferences_viewed: ['notifications_enabled'],
  recording_notification_preference_changed: ['enabled'],
  recording_started: [],
  recording_stopped: ['duration_seconds'],
  session_ended: ['session_duration', 'session_duration_seconds', 'meetings_in_session', 'platform', 'architecture'],
  session_started: ['days_since_last_meeting', 'total_meetings', 'platform', 'architecture'],
  settings_changed: ['setting_type'],
  storage_folder_opened: ['folder_type'],
  summary_copied: ['copy_type', 'copy_count_today', 'has_markdown', 'platform', 'architecture'],
  summary_exported: ['export_format', 'platform', 'architecture'],
  summary_generation_completed: ['model_provider', 'model_name', 'success', 'duration_seconds'],
  summary_generation_started: [
    'model_provider',
    'model_name',
    'transcript_length',
    'time_since_recording_minutes',
    'platform',
    'architecture',
  ],
  summary_regenerated: ['model_provider', 'model_name'],
  system_audio_selected: ['device_category', 'is_bluetooth', 'has_microphone'],
  transcript_copied: [
    'copy_type',
    'copy_count_today',
    'transcript_length',
    'word_count',
    'platform',
    'architecture',
  ],
  transcription_error: [],
  transcription_success: ['duration'],
  user_activated: ['meetings_count', 'days_since_install'],
  user_first_launch: [],
  user_id_copied: [],
};

const BOOLEAN_PROPERTY_KEYS = new Set([
  'enabled',
  'had_fatal_error',
  'has_markdown',
  'has_microphone',
  'has_preferred_microphone',
  'has_preferred_system_audio',
  'has_system_audio',
  'is_auto_detect',
  'is_auto_translate',
  'is_bluetooth',
  'is_first_use',
  'notifications_enabled',
  'success',
]);

const NUMERIC_PROPERTY_KEYS = new Set([
  'active_duration_seconds',
  'chunks_processed',
  'copy_count_today',
  'day_of_week',
  'days_since_install',
  'days_since_last_meeting',
  'duration',
  'duration_seconds',
  'file_size_bytes',
  'hour_of_day',
  'meetings_in_session',
  'meetings_today',
  'meetings_count',
  'pause_duration_seconds',
  'prompt_length',
  'segments_count',
  'session_duration',
  'session_duration_seconds',
  'time_since_recording_minutes',
  'total_duration_seconds',
  'total_meetings',
  'transcript_length',
  'transcript_segments',
  'transcript_segments_count',
  'transcript_word_count',
  'word_count',
  'words_per_minute',
]);

const PROPERTY_KEY_ALIASES: Readonly<Record<string, string>> = {
  model_name: 'model_family',
  old_model: 'old_model_family',
  new_model: 'new_model_family',
  transcription_model: 'transcription_model_family',
  summary_model: 'summary_model_family',
};

const CATEGORICAL_VALUES: Readonly<Record<string, readonly string[]>> = {
  platform: ['Windows', 'macOS', 'Linux', 'tauri', 'unknown'],
  architecture: ['x86', 'x86_64', 'aarch64', 'unknown'],
  device_category: ['default', 'airpods', 'bluetooth', 'wired', 'unknown'],
  microphone_device_type: ['Bluetooth', 'Wired', 'Unknown'],
  system_audio_device_type: ['Bluetooth', 'Wired', 'Unknown'],
  copy_type: ['transcript', 'summary'],
  export_format: ['markdown', 'docx', 'pdf'],
  folder_type: ['database', 'models', 'recordings'],
  setting_type: ['model_config', 'transcript_config'],
  model_provider: [
    'ollama', 'groq', 'claude', 'anthropic', 'openai', 'openrouter', 'builtin-ai',
    'custom-openai', 'parakeet', 'whisper', 'local', 'none', 'unknown',
  ],
  old_provider: [
    'ollama', 'groq', 'claude', 'anthropic', 'openai', 'openrouter', 'builtin-ai',
    'custom-openai', 'parakeet', 'whisper', 'local', 'none', 'unknown',
  ],
  new_provider: [
    'ollama', 'groq', 'claude', 'anthropic', 'openai', 'openrouter', 'builtin-ai',
    'custom-openai', 'parakeet', 'whisper', 'local', 'none', 'unknown',
  ],
  transcription_provider: [
    'ollama', 'groq', 'claude', 'anthropic', 'openai', 'openrouter', 'builtin-ai',
    'custom-openai', 'parakeet', 'whisper', 'local', 'none', 'unknown',
  ],
  summary_provider: [
    'ollama', 'groq', 'claude', 'anthropic', 'openai', 'openrouter', 'builtin-ai',
    'custom-openai', 'parakeet', 'whisper', 'local', 'none', 'unknown',
  ],
  feature: ['importAndRetranscribe', 'structuredSummaries'],
  feature_name: ['template_selected'],
  page: ['home', 'meeting_details'],
  button: [
    'copy_summary',
    'copy_transcript',
    'edit_meeting_title',
    'enhance_transcript',
    'find_in_summary',
    'generate_summary',
    'open_recording_folder',
    'pause_recording',
    'recording_notification_acknowledged',
    'regenerate_summary',
    'replay_product_tour',
    'resume_recording',
    'save_changes',
    'start_recording',
    'start_recording_blocked_downloading',
    'start_recording_blocked_license',
    'start_recording_blocked_missing',
    'start_recording_error',
    'stop_recording',
    'stop_summary_generation',
    'view_meeting_from_toast',
  ],
  location: [
    'home_page', 'meeting_details', 'recording_complete', 'recording_controls',
    'settings', 'sidebar', 'sidebar_auto', 'sidebar_direct', 'toast',
  ],
  error_type: ['import_audio_failed', 'enhance_transcript_failed'],
};

const MODEL_FAMILIES = [
  'parakeet', 'whisper', 'llama', 'qwen', 'gemma', 'mistral', 'deepseek',
  'claude', 'openai', 'phi', 'command-r', 'custom',
] as const;

function bucketModelFamily(rawValue: string): string | null {
  const value = rawValue.trim().toLowerCase();
  if (!value || value.length > 256 || /[\x00-\x1F\x7F]/.test(value)) return null;
  if (value.includes('parakeet')) return 'parakeet';
  if (value.includes('whisper')) return 'whisper';
  if (value.includes('llama')) return 'llama';
  if (value.includes('qwen')) return 'qwen';
  if (value.includes('gemma')) return 'gemma';
  if (value.includes('mistral') || value.includes('mixtral')) return 'mistral';
  if (value.includes('deepseek')) return 'deepseek';
  if (value.includes('claude')) return 'claude';
  if (value.includes('gpt') || /^(o1|o3|o4)/.test(value)) return 'openai';
  if (value.includes('phi')) return 'phi';
  if (value.includes('command-r')) return 'command-r';
  return 'custom';
}

function bucketLanguage(rawValue: string): string | null {
  const value = rawValue.trim();
  if (value === 'auto' || value === 'auto-translate' || value === 'specified') return value;
  return /^[a-z]{2}(?:-[A-Za-z]{2})?$/.test(value) ? 'specified' : null;
}

function sanitizePropertyValue(key: string, rawValue: string): string | null {
  const value = rawValue.trim();
  if (!value || value.length > 96 || /[\x00-\x1F\x7F]/.test(value)) return null;

  if (BOOLEAN_PROPERTY_KEYS.has(key)) {
    return value === 'true' || value === 'false' ? value : null;
  }

  if (NUMERIC_PROPERTY_KEYS.has(key)) {
    if ((key === 'days_since_install' || key === 'days_since_last_meeting') && value === 'null') {
      return value;
    }
    return Number.isFinite(Number(value)) ? value : null;
  }

  if (key === 'app_version') {
    return value === 'unknown' || (/^[0-9][0-9.+-]{0,31}$/.test(value)) ? value : null;
  }
  if (key === 'language' || key === 'language_code') return bucketLanguage(value);
  if (
    key === 'model_family' ||
    key === 'old_model_family' ||
    key === 'new_model_family' ||
    key === 'transcription_model_family' ||
    key === 'summary_model_family'
  ) {
    const family = bucketModelFamily(value);
    return family && MODEL_FAMILIES.includes(family as typeof MODEL_FAMILIES[number]) ? family : null;
  }

  const allowedValues = CATEGORICAL_VALUES[key];
  return allowedValues?.includes(value) ? value : null;
}

function sanitizeEventProperties(
  eventName: string,
  properties: AnalyticsProperties = {},
): AnalyticsProperties | null {
  const eventKeys = EVENT_PROPERTY_ALLOWLIST[eventName];
  if (!eventKeys) return null;

  const allowedKeys = new Set([...eventKeys, 'app_version', 'session_duration']);
  return Object.entries(properties).reduce<AnalyticsProperties>((safeProperties, [key, value]) => {
    if (!allowedKeys.has(key)) return safeProperties;
    const outputKey = PROPERTY_KEY_ALIASES[key] ?? key;
    const safeValue = sanitizePropertyValue(outputKey, String(value));
    if (safeValue !== null) safeProperties[outputKey] = safeValue;
    return safeProperties;
  }, {});
}

function sanitizeProfileProperties(properties: AnalyticsProperties = {}): AnalyticsProperties {
  return Object.entries(properties).reduce<AnalyticsProperties>((safeProperties, [key, value]) => {
    if (!PROFILE_PROPERTY_KEYS.has(key)) return safeProperties;
    const safeValue = sanitizePropertyValue(key, String(value));
    if (safeValue !== null) safeProperties[key] = safeValue;
    return safeProperties;
  }, {});
}

export class Analytics {
  private static initialized = false;
  private static currentUserId: string | null = null;
  private static initializationPromise: Promise<void> | null = null;
  private static sessionStartTime: number | null = null;
  private static meetingsInSession: number = 0;
  private static deviceInfo: DeviceInfo | null = null;

  static async init(): Promise<void> {
    // Prevent duplicate initialization
    if (this.initialized) {
      return;
    }

    // If already initializing, wait for it to complete
    if (this.initializationPromise) {
      return this.initializationPromise;
    }

    this.initializationPromise = this.doInit();
    return this.initializationPromise;
  }

  private static async doInit(): Promise<void> {
    try {
      await invoke('init_analytics');
      this.initialized = true;
      console.log('Analytics initialized successfully');
    } catch (error) {
      console.error('Failed to initialize analytics:', error);
      throw error;
    } finally {
      this.initializationPromise = null;
    }
  }

  static async disable(): Promise<void> {
    try {
      await invoke('disable_analytics');
      this.initialized = false;
      this.currentUserId = null;
      this.initializationPromise = null;
      console.log('Analytics disabled successfully');
    } catch (error) {
      console.error('Failed to disable analytics:', error);
    }
  }

  static async isEnabled(): Promise<boolean> {
    try {
      return await invoke('is_analytics_enabled');
    } catch (error) {
      console.error('Failed to check analytics status:', error);
      return false;
    }
  }

  static async track(eventName: string, properties?: AnalyticsProperties): Promise<void> {
    if (!this.initialized) {
      console.warn('Analytics not initialized');
      return;
    }

    const safeProperties = sanitizeEventProperties(eventName, properties);
    if (safeProperties === null) {
      console.warn('Analytics event blocked by the content-free schema');
      return;
    }

    try {
      await invoke('track_event', { eventName, properties: safeProperties });
    } catch (error) {
      console.error('Failed to track analytics event:', error);
    }
  }

  static async identify(userId: string, properties?: AnalyticsProperties): Promise<void> {
    if (!this.initialized) {
      console.warn('Analytics not initialized');
      return;
    }

    if (!isValidInstallationId(userId)) {
      console.warn('Analytics installation identifier blocked by the content-free schema');
      return;
    }

    try {
      await invoke('identify_user', {
        userId,
        properties: sanitizeProfileProperties(properties),
      });
      this.currentUserId = userId;
    } catch (error) {
      console.error('Failed to identify analytics installation:', error);
    }
  }

  // Enhanced user tracking methods for Phase 1
  static async startSession(userId: string): Promise<string | null> {
    if (!this.initialized) {
      console.warn('Analytics not initialized');
      return null;
    }

    try {
      const sessionId = await invoke('start_analytics_session', { userId });
      this.currentUserId = userId;
      
      return sessionId as string;
    } catch (error) {
      console.error('Failed to start analytics session:', error);
      return null;
    }
  }

  static async endSession(): Promise<void> {
    if (!this.initialized) return;

    try {
      await invoke('end_analytics_session');
    } catch (error) {
      console.error('Failed to end analytics session:', error);
    }
  }

  static async trackDailyActiveUser(): Promise<void> {
    if (!this.initialized) return;

    try {
      await invoke('track_daily_active_user');
    } catch (error) {
      console.error('Failed to track daily active user:', error);
    }
  }

  static async trackUserFirstLaunch(): Promise<void> {
    if (!this.initialized) return;

    try {
      await invoke('track_user_first_launch');
    } catch (error) {
      console.error('Failed to track user first launch:', error);
    }
  }

  static async isSessionActive(): Promise<boolean> {
    if (!this.initialized) return false;

    try {
      return await invoke('is_analytics_session_active');
    } catch (error) {
      console.error('Failed to check session status:', error);
      return false;
    }
  }

  // User ID management with persistent storage
  static async getPersistentUserId(): Promise<string> {
    try {
      // First check if we have a stored user ID
      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load('analytics.json');
      
      let userId = await store.get<string>('user_id');
      
      if (!userId) {
        // Generate new user ID
        userId = `user_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
        await store.set('user_id', userId);
        await store.set('is_first_launch', true);
        await store.save();
      }
      
      return userId;
    } catch (error) {
      console.error('Failed to get persistent user ID:', error);
      // Fallback to session storage
      let userId = sessionStorage.getItem('mityu_user_id');
      if (!userId) {
        userId = `user_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
        sessionStorage.setItem('mityu_user_id', userId);
        sessionStorage.setItem('is_first_launch', 'true');
      }
      return userId;
    }
  }

  static async checkAndTrackFirstLaunch(): Promise<void> {
    try {
      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load('analytics.json');
      
      const isFirstLaunch = await store.get<boolean>('is_first_launch');
      
      if (isFirstLaunch) {
        await this.trackUserFirstLaunch();
        await store.set('is_first_launch', false);
        await store.save();
      }
    } catch (error) {
      console.error('Failed to check first launch:', error);
      // Fallback to session storage
      const isFirstLaunch = sessionStorage.getItem('is_first_launch') === 'true';
      if (isFirstLaunch) {
        await this.trackUserFirstLaunch();
        sessionStorage.removeItem('is_first_launch');
      }
    }
  }

  static async checkAndTrackDailyUsage(): Promise<void> {
    try {
      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load('analytics.json');
      
      const today = new Date().toISOString().split('T')[0];
      const lastTrackedDate = await store.get<string>('last_daily_tracked');
      
      if (lastTrackedDate !== today) {
        await this.trackDailyActiveUser();
        await store.set('last_daily_tracked', today);
        await store.save();
      }
    } catch (error) {
      console.error('Failed to check daily usage:', error);
    }
  }

  static getCurrentUserId(): string | null {
    return this.currentUserId;
  }

  // Platform/Device detection methods
  static async getPlatform(): Promise<string> {
    try {
      // Use browser's user agent as fallback
      const userAgent = navigator.userAgent.toLowerCase();
      if (userAgent.includes('mac')) return 'macOS';
      if (userAgent.includes('win')) return 'Windows';
      if (userAgent.includes('linux')) return 'Linux';
      return 'unknown';
    } catch (error) {
      console.error('Failed to get platform:', error);
      return 'unknown';
    }
  }

  static async getOSVersion(): Promise<string> {
    // Exact OS/user-agent strings are intentionally not collected. Keep this
    // compatibility field coarse for older callers until it can be removed.
    return this.getPlatform();
  }

  static async getDeviceInfo(): Promise<DeviceInfo> {
    if (this.deviceInfo) return this.deviceInfo;

    try {
      const platform = await this.getPlatform();
      const osVersion = await this.getOSVersion();

      // Detect architecture from user agent
      const userAgent = navigator.userAgent.toLowerCase();
      let architecture = 'unknown';
      if (userAgent.includes('arm') || userAgent.includes('aarch64')) {
        architecture = 'aarch64';
      } else if (userAgent.includes('x86_64') || userAgent.includes('x64')) {
        architecture = 'x86_64';
      } else if (userAgent.includes('x86')) {
        architecture = 'x86';
      }

      this.deviceInfo = {
        platform: platform,
        os_version: osVersion,
        architecture: architecture
      };

      return this.deviceInfo;
    } catch (error) {
      console.error('Failed to get device info:', error);
      return {
        platform: 'unknown',
        os_version: 'unknown',
        architecture: 'unknown'
      };
    }
  }

  // Helper methods for analytics.json store
  static async calculateDaysSince(dateKey: string): Promise<number | null> {
    try {
      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load('analytics.json');
      const dateStr = await store.get<string>(dateKey);
      if (!dateStr) return null;
      const diffMs = Date.now() - new Date(dateStr).getTime();
      return Math.floor(diffMs / (1000 * 60 * 60 * 24));
    } catch (error) {
      console.error(`Failed to calculate days since ${dateKey}:`, error);
      return null;
    }
  }

  static async updateMeetingCount(): Promise<void> {
    try {
      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load('analytics.json');

      const totalMeetings = (await store.get<number>('total_meetings') || 0) + 1;
      await store.set('total_meetings', totalMeetings);
      await store.set('last_meeting_date', new Date().toISOString());

      // Update daily count
      const today = new Date().toISOString().split('T')[0];
      const dailyCounts = await store.get<Record<string, number>>('daily_meeting_counts') || {};
      dailyCounts[today] = (dailyCounts[today] || 0) + 1;
      await store.set('daily_meeting_counts', dailyCounts);
      await store.save();
    } catch (error) {
      console.error('Failed to update meeting count:', error);
    }
  }

  static async getMeetingsCountToday(): Promise<number> {
    try {
      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load('analytics.json');
      const today = new Date().toISOString().split('T')[0];
      const dailyCounts = await store.get<Record<string, number>>('daily_meeting_counts') || {};
      return dailyCounts[today] || 0;
    } catch (error) {
      console.error('Failed to get meetings count today:', error);
      return 0;
    }
  }

  static async hasUsedFeatureBefore(featureName: string): Promise<boolean> {
    try {
      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load('analytics.json');
      const features = await store.get<Record<string, any>>('features_used') || {};
      return !!features[featureName];
    } catch (error) {
      console.error(`Failed to check feature usage for ${featureName}:`, error);
      return false;
    }
  }

  static async markFeatureUsed(featureName: string): Promise<void> {
    try {
      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load('analytics.json');
      const features = await store.get<Record<string, any>>('features_used') || {};

      if (!features[featureName]) {
        features[featureName] = {
          first_used: new Date().toISOString(),
          use_count: 1
        };
      } else {
        features[featureName].use_count++;
      }

      await store.set('features_used', features);
      await store.save();
    } catch (error) {
      console.error(`Failed to mark feature used for ${featureName}:`, error);
    }
  }

  // Enhanced session tracking with platform info
  static async trackSessionStarted(_sessionId: string): Promise<void> {
    if (!this.initialized) return;

    try {
      const deviceInfo = await this.getDeviceInfo();
      const daysSinceLast = await this.calculateDaysSince('last_meeting_date');

      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load('analytics.json');
      const totalMeetings = await store.get<number>('total_meetings') || 0;

      this.sessionStartTime = Date.now();
      this.meetingsInSession = 0;

      await this.track('session_started', {
        days_since_last_meeting: daysSinceLast?.toString() || 'null',
        total_meetings: totalMeetings.toString(),
        platform: deviceInfo.platform,
        architecture: deviceInfo.architecture
      });
    } catch (error) {
      console.error('Failed to track session started:', error);
    }
  }

  static async trackSessionEnded(_sessionId: string): Promise<void> {
    if (!this.initialized || !this.sessionStartTime) return;

    try {
      const deviceInfo = await this.getDeviceInfo();
      const sessionDuration = (Date.now() - this.sessionStartTime) / 1000; // seconds

      await this.track('session_ended', {
        session_duration_seconds: sessionDuration.toString(),
        meetings_in_session: this.meetingsInSession.toString(),
        platform: deviceInfo.platform,
        architecture: deviceInfo.architecture
      });
    } catch (error) {
      console.error('Failed to track session ended:', error);
    }
  }

  // Enhanced meeting completion tracking
  static async trackMeetingCompleted(metrics: {
    duration_seconds: number;
    transcript_segments: number;
    transcript_word_count: number;
    words_per_minute: number;
    meetings_today: number;
  }): Promise<void> {
    if (!this.initialized) return;

    try {
      const deviceInfo = await this.getDeviceInfo();

      await this.track('meeting_completed', {
        duration_seconds: metrics.duration_seconds.toString(),
        transcript_segments: metrics.transcript_segments.toString(),
        transcript_word_count: metrics.transcript_word_count.toString(),
        words_per_minute: metrics.words_per_minute.toFixed(2),
        meetings_today: metrics.meetings_today.toString(),
        day_of_week: new Date().getDay().toString(),
        hour_of_day: new Date().getHours().toString(),
        platform: deviceInfo.platform,
        architecture: deviceInfo.architecture
      });

      this.meetingsInSession++;
    } catch (error) {
      console.error('Failed to track meeting completed:', error);
    }
  }

  // Feature usage tracking with platform info
  static async trackFeatureUsedEnhanced(featureName: string, properties?: Record<string, any>): Promise<void> {
    if (!this.initialized) return;

    try {
      const deviceInfo = await this.getDeviceInfo();
      const isFirstUse = !(await this.hasUsedFeatureBefore(featureName));
      await this.markFeatureUsed(featureName);

      const trackingProperties: AnalyticsProperties = {
        feature_name: featureName,
        is_first_use: isFirstUse.toString(),
        platform: deviceInfo.platform,
        architecture: deviceInfo.architecture
      };

      // Add additional properties if provided
      if (properties) {
        Object.entries(properties).forEach(([key, value]) => {
          trackingProperties[key] = String(value);
        });
      }

      await this.track('feature_used', trackingProperties);
    } catch (error) {
      console.error(`Failed to track feature used: ${featureName}`, error);
    }
  }

  // Copy tracking with frequency
  static async trackCopy(copyType: 'transcript' | 'summary', properties?: Record<string, any>): Promise<void> {
    if (!this.initialized) return;

    try {
      const deviceInfo = await this.getDeviceInfo();
      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load('analytics.json');

      // Get today's date
      const today = new Date().toISOString().split('T')[0];
      const copyCounts = await store.get<Record<string, any>>('copy_counts') || {};
      const todayCounts = copyCounts[today] || {};
      const copyCount = todayCounts[copyType] || 0;

      // Update copy count
      todayCounts[copyType] = copyCount + 1;
      copyCounts[today] = todayCounts;
      await store.set('copy_counts', copyCounts);
      await store.save();

      const trackingProperties: AnalyticsProperties = {
        copy_type: copyType,
        copy_count_today: (copyCount + 1).toString(),
        platform: deviceInfo.platform,
        architecture: deviceInfo.architecture
      };

      // Add additional properties if provided
      if (properties) {
        Object.entries(properties).forEach(([key, value]) => {
          trackingProperties[key] = String(value);
        });
      }

      await this.track(`${copyType}_copied`, trackingProperties);
    } catch (error) {
      console.error(`Failed to track ${copyType} copy:`, error);
    }
  }

  // Export tracking is content-free by construction: only format and coarse
  // platform metadata are accepted by the shared analytics schema.
  static async trackExport(properties: {
    format: 'markdown' | 'docx' | 'pdf';
  }): Promise<void> {
    if (!this.initialized) return;

    try {
      const deviceInfo = await this.getDeviceInfo();

      const trackingProperties: AnalyticsProperties = {
        export_format: properties.format,
        platform: deviceInfo.platform,
        architecture: deviceInfo.architecture,
      };

      await this.track('summary_exported', trackingProperties);
    } catch (error) {
      console.error('Failed to track export:', error);
    }
  }

  // Meeting-specific tracking methods
  static async trackMeetingStarted(): Promise<void> {
    if (!this.initialized) return;

    try {
      await invoke('track_meeting_started');
    } catch (error) {
      console.error('Failed to track meeting started:', error);
    }
  }

  static async trackRecordingStarted(): Promise<void> {
    if (!this.initialized) return;

    try {
      await invoke('track_recording_started');
    } catch (error) {
      console.error('Failed to track recording started:', error);
    }
  }

  static async trackRecordingStopped(durationSeconds?: number): Promise<void> {
    if (!this.initialized) return;

    try {
      await invoke('track_recording_stopped', { durationSeconds });
    } catch (error) {
      console.error('Failed to track recording stopped:', error);
    }
  }

  static async trackMeetingDeleted(): Promise<void> {
    if (!this.initialized) return;

    try {
      await invoke('track_meeting_deleted');
    } catch (error) {
      console.error('Failed to track meeting deleted:', error);
    }
  }

  static async trackSettingsChanged(settingType: string): Promise<void> {
    if (!this.initialized) return;

    try {
      await invoke('track_settings_changed', { settingType });
    } catch (error) {
      console.error('Failed to track settings changed:', error);
    }
  }

  static async trackFeatureUsed(featureName: string): Promise<void> {
    if (!this.initialized) return;

    try {
      await invoke('track_feature_used', { featureName });
    } catch (error) {
      console.error('Failed to track feature used:', error);
    }
  }

  // Convenience methods for common events
  static async trackPageView(pageName: string): Promise<void> {
    await this.track('page_view', { page: pageName });
  }

  static async trackButtonClick(buttonName: string, location?: string): Promise<void> {
    const properties: AnalyticsProperties = { button: buttonName };
    if (location) properties.location = location;
    await this.track('button_click', properties);
  }

  static async trackError(errorType: string): Promise<void> {
    await this.track('error', { error_type: errorType });
  }

  static async trackAppStarted(): Promise<void> {
    await this.track('app_started');
  }

  // Cleanup method for app shutdown
  static async cleanup(): Promise<void> {
    await this.endSession();
  }

  // Reset initialization state (useful for testing)
  static reset(): void {
    this.initialized = false;
    this.currentUserId = null;
    this.initializationPromise = null;
  }

  // Wait for analytics to be initialized
  static async waitForInitialization(timeout: number = 5000): Promise<boolean> {
    if (this.initialized) {
      return true;
    }
    
    const startTime = Date.now();
    while (!this.initialized && (Date.now() - startTime) < timeout) {
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    
    return this.initialized;
  }

  // Track backend connection success/failure
  static async trackBackendConnection(success: boolean) {
    // Wait for analytics to be initialized
    const isInitialized = await this.waitForInitialization();
    if (!isInitialized) {
      console.warn('Analytics not initialized within timeout, skipping backend connection tracking');
      return;
    }

    await this.track('backend_connection', { success: success.toString() });
  }

  // Track transcription errors
  static async trackTranscriptionError() {
    if (!this.initialized) {
      console.warn('Analytics not initialized, skipping transcription error tracking');
      return;
    }

    await this.track('transcription_error');
  }

  // Track transcription success
  static async trackTranscriptionSuccess(duration?: number) {
    if (!this.initialized) {
      console.warn('Analytics not initialized, skipping transcription success tracking');
      return;
    }

    const properties = duration === undefined ? undefined : { duration: duration.toString() };
    await this.track('transcription_success', properties);
  }

  // Summary generation analytics
  static async trackSummaryGenerationStarted(
    modelProvider: string,
    modelName: string,
    transcriptLength: number,
    timeSinceRecordingMinutes?: number
  ) {
    if (!this.initialized) {
      console.warn('Analytics not initialized, skipping summary generation started tracking');
      return;
    }

    try {
      const deviceInfo = await this.getDeviceInfo();
      console.log('Tracking summary generation started event:', {
        modelProvider,
        modelName,
        transcriptLength,
        timeSinceRecordingMinutes
      });

      const properties: AnalyticsProperties = {
        model_provider: modelProvider,
        model_name: modelName,
        transcript_length: transcriptLength.toString(),
        platform: deviceInfo.platform,
        architecture: deviceInfo.architecture
      };

      if (timeSinceRecordingMinutes !== undefined) {
        properties.time_since_recording_minutes = timeSinceRecordingMinutes.toFixed(2);
      }

      await this.track('summary_generation_started', properties);
      console.log('Summary generation started event tracked successfully');
    } catch (error) {
      console.error('Failed to track summary generation started:', error);
    }
  }

  static async trackSummaryGenerationCompleted(
    modelProvider: string, 
    modelName: string, 
    success: boolean, 
    durationSeconds?: number
  ) {
    if (!this.initialized) {
      console.warn('Analytics not initialized, skipping summary generation completed tracking');
      return;
    }

    try {
      console.log('Tracking summary generation completed event:', { modelProvider, modelName, success, durationSeconds });
      await invoke('track_summary_generation_completed', {
        modelProvider,
        modelName,
        success,
        durationSeconds
      });
      console.log('Summary generation completed event tracked successfully');
    } catch (error) {
      console.error('Failed to track summary generation completed:', error);
    }
  }

  static async trackSummaryRegenerated(modelProvider: string, modelName: string) {
    if (!this.initialized) {
      console.warn('Analytics not initialized, skipping summary regenerated tracking');
      return;
    }

    try {
      console.log('Tracking summary regenerated event:', { modelProvider, modelName });
      await invoke('track_summary_regenerated', {
        modelProvider,
        modelName
      });
      console.log('Summary regenerated event tracked successfully');
    } catch (error) {
      console.error('Failed to track summary regenerated:', error);
    }
  }

  static async trackModelChanged(oldProvider: string, oldModel: string, newProvider: string, newModel: string) {
    if (!this.initialized) {
      console.warn('Analytics not initialized, skipping model changed tracking');
      return;
    }

    try {
      console.log('Tracking model changed event:', { oldProvider, oldModel, newProvider, newModel });
      await invoke('track_model_changed', {
        oldProvider,
        oldModel,
        newProvider,
        newModel
      });
      console.log('Model changed event tracked successfully');
    } catch (error) {
      console.error('Failed to track model changed:', error);
    }
  }

  static async trackCustomPromptUsed(promptLength: number) {
    if (!this.initialized) {
      console.warn('Analytics not initialized, skipping custom prompt used tracking');
      return;
    }

    try {
      console.log('Tracking custom prompt used event:', { promptLength });
      await invoke('track_custom_prompt_used', {
        promptLength
      });
      console.log('Custom prompt used event tracked successfully');
    } catch (error) {
      console.error('Failed to track custom prompt used:', error);
    }
  }
}

export default Analytics;

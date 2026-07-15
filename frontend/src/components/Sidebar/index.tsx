'use client';

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { ChevronDown, ChevronRight, File, Settings, PanelLeftClose, PanelLeftOpen, Calendar, Home, Trash2, Mic, Square, Plus, Pencil, NotebookPen, SearchIcon, X, Upload, ListChecks } from 'lucide-react';
import { useRouter, usePathname } from 'next/navigation';
import { useSidebar } from './SidebarProvider';
import type { CurrentMeeting } from '@/components/Sidebar/SidebarProvider';
import { ConfirmationModal } from '../ConfirmationModel/confirmation-modal';
import { ModelConfig } from '@/components/ModelSettingsModal';
import { SettingTabs } from '../SettingTabs';
import { TranscriptModelProps } from '@/components/TranscriptSettings';
import Analytics from '@/lib/analytics';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { isTauri } from '@/lib/isTauri';
import { configService } from '@/services/configService';
import { indexedDBService } from '@/services/indexedDBService';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { toast } from 'sonner';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { useImportDialog } from '@/contexts/ImportDialogContext';
import { useConfig } from '@/contexts/ConfigContext';
import { TOUR_ANCHORS } from '@/lib/tour';
import { APP_VERSION } from '@/lib/appVersion';

import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog"
import { VisuallyHidden } from "@/components/ui/visually-hidden"

import { MessageToast } from '../MessageToast';
import Logo from '../Logo';
import Info from '../Info';
import { ComplianceNotification } from '../ComplianceNotification';
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from '../ui/input-group';
import { SearchResultsList } from './SearchResultsList';
import {
  isEvidenceQuerySearchable,
  type TranscriptSearchResult,
} from '@/services/search';

interface SidebarItem {
  id: string;
  title: string;
  type: 'folder' | 'file';
  children?: SidebarItem[];
}

const Sidebar: React.FC = () => {
  const router = useRouter();
  const pathname = usePathname();
  const searchJumpNonceRef = useRef(0);
  const {
    currentMeeting,
    setCurrentMeeting,
    sidebarItems,
    isCollapsed,
    toggleCollapse,
    handleRecordingToggle,
    searchTranscripts,
    searchResults,
    isSearching,
    searchError,
    meetings,
    setMeetings,
    serverAddress
  } = useSidebar();

  // Get recording state from RecordingStateContext (single source of truth)
  const { isRecording } = useRecordingState();
  const { openImportDialog } = useImportDialog();
  const { betaFeatures } = useConfig();
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set(['meetings']));
  const [searchQuery, setSearchQuery] = useState<string>('');
  const [showModelSettings, setShowModelSettings] = useState(false);
  // App version for the footer — read from the Tauri app (tauri.conf.json), so it
  // never goes stale on a release bump. Plain-browser renders fall back to the
  // synchronized package-manifest version when the Tauri API is absent.
  const [appVersion, setAppVersion] = useState(APP_VERSION);
  useEffect(() => {
    if (isTauri()) getVersion().then(setAppVersion).catch(() => {});
  }, []);
  const [modelConfig, setModelConfig] = useState<ModelConfig>({
    provider: 'ollama',
    model: '',
    whisperModel: '',
    apiKey: null,
    ollamaEndpoint: null
  });
  const [transcriptModelConfig, setTranscriptModelConfig] = useState<TranscriptModelProps>({
    provider: 'parakeet',
    model: 'parakeet-tdt-0.6b-v3-int8',
  });
  const [settingsSaveSuccess, setSettingsSaveSuccess] = useState<boolean | null>(null);

  // State for edit modal
  const [editModalState, setEditModalState] = useState<{ isOpen: boolean; meetingId: string | null; currentTitle: string }>({
    isOpen: false,
    meetingId: null,
    currentTitle: ''
  });
  const [editingTitle, setEditingTitle] = useState<string>('');

  // Ensure 'meetings' folder is always expanded
  useEffect(() => {
    if (!expandedFolders.has('meetings')) {
      const newExpanded = new Set(expandedFolders);
      newExpanded.add('meetings');
      setExpandedFolders(newExpanded);
    }
  }, [expandedFolders]);

  // useEffect(() => {
  //   if (settingsSaveSuccess !== null) {
  //     const timer = setTimeout(() => {
  //       setSettingsSaveSuccess(null);
  //     }, 3000);
  //   }
  // }, [settingsSaveSuccess]);


  const [deleteModalState, setDeleteModalState] = useState<{ isOpen: boolean; itemId: string | null }>({ isOpen: false, itemId: null });
  const [isDeletePending, setIsDeletePending] = useState(false);

  useEffect(() => {
    // Note: Don't set hardcoded defaults - let DB be the source of truth
    const fetchModelConfig = async () => {
      // Only make API call if serverAddress is loaded
      if (!serverAddress) {
        console.log('Waiting for server address to load before fetching model config');
        return;
      }

      try {
        const data = await configService.getModelConfig() as any;
        if (data && data.provider !== null) {
          setModelConfig(data);
        }
      } catch (error) {
        console.error('Failed to fetch model config');
      }
    };

    fetchModelConfig();
  }, [serverAddress]);


  useEffect(() => {
    // Note: Don't set hardcoded defaults - let DB be the source of truth
    const fetchTranscriptSettings = async () => {
      // Only make API call if serverAddress is loaded
      if (!serverAddress) {
        console.log('Waiting for server address to load before fetching transcript settings');
        return;
      }

      try {
        const data = await invoke('api_get_transcript_config') as any;
        if (data && data.provider !== null) {
          setTranscriptModelConfig(data);
        }
      } catch (error) {
        console.error('Failed to fetch transcript settings');
      }
    };
    fetchTranscriptSettings();
  }, [serverAddress]);

  // Listen for model config updates from other components
  useEffect(() => {
    const setupListener = async () => {
      const { listen } = await import('@tauri-apps/api/event');
      const unlisten = await listen<ModelConfig>('model-config-updated', (event) => {
        console.log('Sidebar received model-config-updated event');
        setModelConfig(event.payload);
      });

      return unlisten;
    };

    let cleanup: (() => void) | undefined;
    setupListener().then(fn => cleanup = fn);

    return () => {
      cleanup?.();
    };
  }, []);



  // Handle model config save
  const handleSaveModelConfig = async (config: ModelConfig) => {
    try {
      await invoke('api_save_model_config', {
        provider: config.provider,
        model: config.model,
        whisperModel: config.whisperModel,
        apiKey: config.apiKey,
        ollamaEndpoint: config.ollamaEndpoint,
      });

      setModelConfig(config);
      console.log('Model config saved successfully');
      setSettingsSaveSuccess(true);

      // Emit event to sync other components
      const { emit } = await import('@tauri-apps/api/event');
      await emit('model-config-updated', config);

      // Track settings change
      await Analytics.trackSettingsChanged('model_config');
    } catch (error) {
      console.error('Error saving model config');
      setSettingsSaveSuccess(false);
    }
  };

  const handleSaveTranscriptConfig = async (updatedConfig?: TranscriptModelProps) => {
    try {
      const configToSave = updatedConfig || transcriptModelConfig;
      const payload = {
        provider: configToSave.provider,
        model: configToSave.model,
        apiKey: configToSave.apiKey ?? null
      };
      console.log('Saving transcript config');

      await invoke('api_save_transcript_config', {
        provider: payload.provider,
        model: payload.model,
        apiKey: payload.apiKey,
      });


      setSettingsSaveSuccess(true);

      // Track settings change
      await Analytics.trackSettingsChanged('transcript_config');
    } catch (error) {
      console.error('Failed to save transcript config');
      setSettingsSaveSuccess(false);
    }
  };

  // Keep search input responsive while the provider owns debounce, stale
  // response suppression and the local evidence-search command.
  const handleSearchChange = useCallback((value: string) => {
    setSearchQuery(value);
    searchTranscripts(value);

    if (value.trim()) {
      setExpandedFolders((current) => {
        if (current.has('meetings')) return current;
        const next = new Set(current);
        next.add('meetings');
        return next;
      });
    }
  }, [searchTranscripts]);

  const handleSearchResultSelect = useCallback((result: TranscriptSearchResult) => {
    setCurrentMeeting({ id: result.id, title: result.title });
    const jumpNonce = ++searchJumpNonceRef.current;
    const params = new URLSearchParams({
      id: result.id,
      segment: result.sourceChunkId,
      source: 'search',
      jump: `${Date.now()}-${jumpNonce}`,
    });
    router.push(`/meeting-details?${params.toString()}`);
  }, [router, setCurrentMeeting]);


  const handleDelete = async (itemId: string): Promise<boolean> => {
    console.log('Deleting meeting');

    try {
      // Browser recovery data is outside the native SQLite transaction. Purge
      // legacy saved copies and an exact matching recovery ID before asking the
      // native layer to remove its managed database/search/recording data.
      await indexedDBService.purgeSavedMeetings();
      await indexedDBService.deleteMeeting(itemId);
      if (sessionStorage.getItem('indexeddb_current_meeting_id') === itemId) {
        sessionStorage.removeItem('indexeddb_current_meeting_id');
      }

      await invoke('api_delete_meeting', {
        meetingId: itemId,
      });
      console.log('Meeting deleted successfully');
      const updatedMeetings = meetings.filter((m: CurrentMeeting) => m.id !== itemId);
      setMeetings(updatedMeetings);

      // Track meeting deletion
      Analytics.trackMeetingDeleted();

      // Show success toast
      toast.success("Meeting deleted successfully", {
        description: "Mityu-managed database, search, recording, and recovery data was removed."
      });

      // If deleting the active meeting, navigate to home
      if (currentMeeting?.id === itemId) {
        setCurrentMeeting({ id: 'intro-call', title: '+ New Call' });
        router.push('/');
      }

      return true;
    } catch (error) {
      console.error('Failed to delete meeting');
      toast.error("Failed to delete meeting", {
        description: error instanceof Error ? error.message : String(error)
      });
      return false;
    }
  };

  const handleDeleteConfirm = async () => {
    const itemId = deleteModalState.itemId;
    if (!itemId || isDeletePending) return;

    setIsDeletePending(true);
    try {
      const deleted = await handleDelete(itemId);
      if (deleted) {
        setDeleteModalState({ isOpen: false, itemId: null });
      }
    } finally {
      setIsDeletePending(false);
    }
  };

  // Handle modal editing of meeting names
  const handleEditStart = (meetingId: string, currentTitle: string) => {
    setEditModalState({
      isOpen: true,
      meetingId: meetingId,
      currentTitle: currentTitle
    });
    setEditingTitle(currentTitle);
  };

  const handleEditConfirm = async () => {
    const newTitle = editingTitle.trim();
    const meetingId = editModalState.meetingId;

    if (!meetingId) return;

    // Prevent empty titles
    if (!newTitle) {
      toast.error("Meeting title cannot be empty");
      return;
    }

    try {
      await invoke('api_save_meeting_title', {
        meetingId: meetingId,
        title: newTitle,
      });

      // Update local state
      const updatedMeetings = meetings.map((m: CurrentMeeting) =>
        m.id === meetingId ? { ...m, title: newTitle } : m
      );
      setMeetings(updatedMeetings);

      // Update current meeting if it's the one being edited
      if (currentMeeting?.id === meetingId) {
        setCurrentMeeting({ id: meetingId, title: newTitle });
      }

      // Track the edit
      Analytics.trackButtonClick('edit_meeting_title', 'sidebar');

      toast.success("Meeting title updated successfully");

      // Close modal and reset state
      setEditModalState({ isOpen: false, meetingId: null, currentTitle: '' });
      setEditingTitle('');
    } catch (error) {
      console.error('Failed to update meeting title');
      toast.error("Failed to update meeting title", {
        description: error instanceof Error ? error.message : String(error)
      });
    }
  };

  const handleEditCancel = () => {
    setEditModalState({ isOpen: false, meetingId: null, currentTitle: '' });
    setEditingTitle('');
  };

  const toggleFolder = (folderId: string) => {
    // Normal toggle behavior for all folders
    const newExpanded = new Set(expandedFolders);
    if (newExpanded.has(folderId)) {
      newExpanded.delete(folderId);
    } else {
      newExpanded.add(folderId);
    }
    setExpandedFolders(newExpanded);
  };

  // Expose setShowModelSettings to window for Rust tray to call
  useEffect(() => {
    (window as any).openSettings = () => {
      setShowModelSettings(true);
    };

    // Cleanup on unmount
    return () => {
      delete (window as any).openSettings;
    };
  }, []);

  const renderCollapsedIcons = () => {
    if (!isCollapsed) return null;

    const isHomePage = pathname === '/';
    const isActionsPage = pathname === '/actions';
    const isMeetingPage = pathname?.includes('/meeting-details');
    const isSettingsPage = pathname === '/settings';

    return (
      <TooltipProvider>
        <div className="flex flex-col items-center space-y-2 mt-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={() => router.push('/')}
                className={`grid h-10 w-10 place-items-center rounded-xl transition-colors duration-150 ${isHomePage ? 'bg-accent text-primary' : 'text-muted-foreground hover:bg-muted hover:text-foreground'
                  }`}
              >
                <Home className="w-5 h-5" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">
              <p>Home</p>
            </TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                onClick={() => router.push('/actions')}
                aria-label="Open Action Center"
                aria-current={isActionsPage ? 'page' : undefined}
                className={`grid h-10 w-10 place-items-center rounded-xl transition-colors duration-150 ${isActionsPage ? 'bg-accent text-primary' : 'text-muted-foreground hover:bg-muted hover:text-foreground'
                  }`}
              >
                <ListChecks className="h-5 w-5" aria-hidden="true" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">
              <p>Action Center</p>
            </TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                data-tour={TOUR_ANCHORS.recordButton}
                onClick={handleRecordingToggle}
                disabled={isRecording}
                className={`grid h-10 w-10 place-items-center rounded-xl text-white shadow-sm transition-colors duration-150 ${isRecording ? 'bg-red-500 cursor-not-allowed' : 'bg-red-500 hover:bg-red-600'}`}
              >
                {isRecording ? (
                  <Square className="w-5 h-5 text-white" />
                ) : (
                  <Mic className="w-5 h-5 text-white" />
                )}
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">
              <p>{isRecording ? "Recording in progress..." : "Start Recording"}</p>
            </TooltipContent>
          </Tooltip>

          {betaFeatures.importAndRetranscribe && (
            <Tooltip>
              <TooltipTrigger asChild>
                <button
                  onClick={() => openImportDialog()}
                  className="grid h-10 w-10 place-items-center rounded-xl bg-accent text-primary transition-colors duration-150 hover:bg-accent/80"
                >
                  <Upload className="w-5 h-5" />
                </button>
              </TooltipTrigger>
              <TooltipContent side="right">
                <p>Import Audio</p>
              </TooltipContent>
            </Tooltip>
          )}

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={() => {
                  if (isCollapsed) toggleCollapse();
                  toggleFolder('meetings');
                }}
                className={`grid h-10 w-10 place-items-center rounded-xl transition-colors duration-150 ${isMeetingPage ? 'bg-accent text-primary' : 'text-muted-foreground hover:bg-muted hover:text-foreground'
                  }`}
              >
                <NotebookPen className="w-5 h-5" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">
              <p>Meeting Notes</p>
            </TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={() => router.push('/settings')}
                className={`grid h-10 w-10 place-items-center rounded-xl transition-colors duration-150 ${isSettingsPage ? 'bg-accent text-primary' : 'text-muted-foreground hover:bg-muted hover:text-foreground'
                  }`}
              >
                <Settings className="w-5 h-5" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">
              <p>Settings</p>
            </TooltipContent>
          </Tooltip>

          <Info isCollapsed={isCollapsed} />
        </div>
      </TooltipProvider>
    );
  };

  const renderItem = (item: SidebarItem, depth = 0) => {
    const isExpanded = expandedFolders.has(item.id);
    const paddingLeft = `${depth * 12 + 12}px`;
    const isActive = item.type === 'file' && currentMeeting?.id === item.id;
    const isMeetingItem = item.id.includes('-') && !item.id.startsWith('intro-call');

    if (isCollapsed) return null;

    return (
      <div key={item.id}>
        <div
          className={`flex items-center transition-all duration-150 group ${item.type === 'folder' && depth === 0
            ? 'p-3 text-lg font-semibold h-10 mx-3 mt-3 rounded-lg'
            : `relative px-3 py-2 my-0.5 rounded-md text-sm transition-colors ${isActive ? 'bg-primary/10 text-primary font-medium before:absolute before:left-0 before:top-1/2 before:-translate-y-1/2 before:h-5 before:w-[3px] before:rounded-r-full before:bg-primary' :
              'text-foreground/80 hover:bg-muted hover:text-foreground'
            } cursor-pointer`
            }`}
          style={item.type === 'folder' && depth === 0 ? {} : { paddingLeft }}
          onClick={() => {
            if (item.type === 'folder') {
              toggleFolder(item.id);
            } else {
              setCurrentMeeting({ id: item.id, title: item.title });
              const basePath = item.id.startsWith('intro-call') ? '/' :
                item.id.includes('-') ? `/meeting-details?id=${item.id}` : `/notes/${item.id}`;
              router.push(basePath);
            }
          }}
        >
          {item.type === 'folder' ? (
            <>
              {item.id === 'meetings' ? (
                <Calendar className="w-4 h-4 mr-2" />
              ) : item.id === 'notes' ? (
                <Calendar className="w-4 h-4 mr-2" />
              ) : null}
              <span className={depth === 0 ? "" : "font-medium"}>{item.title}</span>
              <div className="ml-auto">
                {isExpanded ? (
                  <ChevronDown className="w-4 h-4 text-muted-foreground" />
                ) : (
                  <ChevronRight className="w-4 h-4 text-muted-foreground" />
                )}
              </div>
              {searchQuery && item.id === 'meetings' && isSearching && (
                <span className="ml-2 text-xs text-primary animate-pulse">Searching...</span>
              )}
            </>
          ) : (
            <div className="flex flex-col w-full">
              <div className="flex items-center w-full">
                {isMeetingItem ? (
                  <div className="flex-shrink-0 flex items-center justify-center w-6 h-6 rounded-full mr-2 bg-muted">
                    <File className="w-3.5 h-3.5 text-muted-foreground" />
                  </div>
                ) : (
                  <div className="flex-shrink-0 flex items-center justify-center w-6 h-6 rounded-full mr-2 bg-accent">
                    <Plus className="w-3.5 h-3.5 text-primary" />
                  </div>
                )}
                <span className="flex-1 break-words">{item.title}</span>
                {isMeetingItem && (
                  <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleEditStart(item.id, item.title);
                      }}
                      className="hover:text-primary p-1 rounded-md hover:bg-accent flex-shrink-0"
                      aria-label="Edit meeting title"
                    >
                      <Pencil className="w-4 h-4" />
                    </button>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setDeleteModalState({ isOpen: true, itemId: item.id });
                      }}
                      className="hover:text-red-600 p-1 rounded-md hover:bg-red-50 flex-shrink-0"
                      aria-label="Delete meeting"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                )}
              </div>

            </div>
          )}
        </div>
        {item.type === 'folder' && isExpanded && item.children && (
          <div className="ml-1">
            {item.children.map(child => renderItem(child, depth + 1))}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="fixed top-0 left-0 h-screen z-40">
      <div
        className={`h-screen bg-card border-r border-border shadow-sm flex flex-col transition-[width] duration-300 ease-out ${isCollapsed ? 'w-16' : 'w-64'
          }`}
      >
        {/* Header: brand · collapse toggle · search */}
        <div className="flex-shrink-0 px-3 pt-4 pb-2">
          {isCollapsed ? (
            <div className="flex flex-col items-center gap-3">
              <Logo isCollapsed />
              <button
                onClick={toggleCollapse}
                title="Expand sidebar"
                aria-label="Expand sidebar"
                className="grid h-9 w-9 place-items-center rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
              >
                <PanelLeftOpen className="h-5 w-5" />
              </button>
            </div>
          ) : (
            <>
              <div className="flex items-center justify-between gap-2 mb-3">
                <Logo isCollapsed={false} />
                <button
                  onClick={toggleCollapse}
                  title="Collapse sidebar"
                  aria-label="Collapse sidebar"
                  className="grid h-8 w-8 shrink-0 place-items-center rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                >
                  <PanelLeftClose className="h-[18px] w-[18px]" />
                </button>
              </div>

              <div className="relative">
                <InputGroup>
                  <InputGroupInput placeholder='Search meeting evidence…' value={searchQuery}
                    aria-label="Search meeting evidence"
                    onChange={(e) => handleSearchChange(e.target.value)}
                  />
                  <InputGroupAddon>
                    <SearchIcon />
                  </InputGroupAddon>
                  {searchQuery &&
                    <InputGroupAddon align={'inline-end'}>
                      <InputGroupButton
                        type="button"
                        aria-label="Clear meeting search"
                        onClick={() => handleSearchChange('')}
                      >
                        <X />
                      </InputGroupButton>
                    </InputGroupAddon>
                  }
                </InputGroup>
              </div>
            </>
          )}
        </div>

        {/* Main content - scrollable area */}
        <div className="flex-1 flex flex-col min-h-0">
          {/* Fixed navigation items */}
          <div className="flex-shrink-0">
            {!isCollapsed && (
              <>
                <div
                  onClick={() => router.push('/')}
                  className="p-3 text-lg font-semibold items-center hover:bg-muted h-10 flex mx-3 mt-3 rounded-lg cursor-pointer"
                >
                  <Home className="w-4 h-4 mr-2" />
                  <span>Home</span>
                </div>
                <button
                  type="button"
                  onClick={() => router.push('/actions')}
                  aria-current={pathname === '/actions' ? 'page' : undefined}
                  className={`mx-3 mt-2 flex h-10 w-[calc(100%_-_1.5rem)] items-center rounded-lg p-3 text-left text-lg font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary ${pathname === '/actions'
                    ? 'bg-accent text-primary'
                    : 'text-foreground hover:bg-muted'
                    }`}
                >
                  <ListChecks className="mr-2 h-4 w-4" aria-hidden="true" />
                  <span>Action Center</span>
                </button>
              </>
            )}
          </div>

          {/* Content area */}
          <div className="flex-1 flex flex-col min-h-0">
            {renderCollapsedIcons()}
            {/* Meeting Notes folder header - fixed */}
            {!isCollapsed && (
              <div className="flex-shrink-0">
                {sidebarItems.filter(item => item.type === 'folder').map(item => (
                  <div key={item.id}>
                    <div
                      className="flex items-center transition-all duration-150 p-3 text-lg font-semibold h-10 mx-3 mt-3 rounded-lg"
                    >
                      <NotebookPen className="w-4 h-4 mr-2 text-muted-foreground" />
                      <span className="text-foreground">{item.title}</span>
                      {searchQuery && item.id === 'meetings' && isSearching && (
                        <span className="ml-2 text-xs text-primary animate-pulse">Searching...</span>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}

            {/* Scrollable meeting items */}
            {!isCollapsed && (
              <div className="flex-1 overflow-y-auto custom-scrollbar min-h-0">
                {searchQuery.trim() ? (
                  <SearchResultsList
                    results={searchResults}
                    isSearching={isSearching}
                    isQueryTooShort={!isEvidenceQuerySearchable(searchQuery)}
                    error={searchError}
                    onSelect={handleSearchResultSelect}
                  />
                ) : (
                  sidebarItems
                    .filter(item => item.type === 'folder' && expandedFolders.has(item.id) && item.children)
                    .map(item => (
                      <div key={`${item.id}-children`} className="mx-3">
                        {item.children!.map(child => renderItem(child, 1))}
                      </div>
                    ))
                )}
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        {!isCollapsed && (

          <div className="flex-shrink-0 p-3 border-t border-border space-y-2">
            {/* Primary CTA */}
            <button
              data-tour={TOUR_ANCHORS.recordButton}
              onClick={handleRecordingToggle}
              disabled={isRecording}
              className={`w-full flex items-center justify-center gap-2 px-3 py-2.5 text-sm font-semibold text-white rounded-xl transition-colors shadow-sm ${isRecording ? 'bg-red-400 cursor-not-allowed' : 'bg-red-500 hover:bg-red-600'}`}
            >
              {isRecording ? (
                <>
                  <span className="h-2 w-2 rounded-full bg-white animate-pulse" />
                  <span>Recording…</span>
                </>
              ) : (
                <>
                  <Mic className="w-4 h-4" />
                  <span>Start recording</span>
                </>
              )}
            </button>

            {/* Secondary actions — compact icon row */}
            <div className="flex items-center gap-1">
              {betaFeatures.importAndRetranscribe && (
                <button
                  onClick={() => openImportDialog()}
                  title="Import audio"
                  aria-label="Import audio"
                  className="flex-1 grid h-9 place-items-center rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                >
                  <Upload className="w-[18px] h-[18px]" />
                </button>
              )}
              <button
                onClick={() => router.push('/settings')}
                title="Settings"
                aria-label="Settings"
                className="flex-1 grid h-9 place-items-center rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
              >
                <Settings className="w-[18px] h-[18px]" />
              </button>
              <div className="flex-1 grid place-items-center [&_button]:mb-0">
                <Info isCollapsed />
              </div>
              <span className="ml-auto pl-1 text-[11px] tabular-nums text-muted-foreground/70">v{appVersion}</span>
            </div>
          </div>
        )}
      </div>

      {/* Confirmation Modal for Delete */}
      <ConfirmationModal
        isOpen={deleteModalState.isOpen}
        text="This removes the meeting from Mityu-managed local database and search data, its Mityu-managed recording artifacts, and recovery cache. Unknown files you placed in the recording folder are retained. This cannot be undone. SSD wear-leveling, copy-on-write filesystems, snapshots, backups, exports, and WebView/browser storage may retain physical traces or separate copies that Mityu cannot erase."
        onConfirm={handleDeleteConfirm}
        onCancel={() => {
          if (!isDeletePending) {
            setDeleteModalState({ isOpen: false, itemId: null });
          }
        }}
        isBusy={isDeletePending}
      />

      {/* Edit Meeting Title Modal */}
      <Dialog open={editModalState.isOpen} onOpenChange={(open) => {
        if (!open) handleEditCancel();
      }}>
        <DialogContent className="sm:max-w-[425px]">
          <VisuallyHidden>
            <DialogTitle>Edit Meeting Title</DialogTitle>
          </VisuallyHidden>
          <div className="py-4">
            <h3 className="text-lg font-semibold mb-4">Edit Meeting Title</h3>
            <div className="space-y-4">
              <div>
                <label htmlFor="meeting-title" className="block text-sm font-medium text-foreground mb-2">
                  Meeting Title
                </label>
                <input
                  id="meeting-title"
                  type="text"
                  value={editingTitle}
                  onChange={(e) => setEditingTitle(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      handleEditConfirm();
                    } else if (e.key === 'Escape') {
                      handleEditCancel();
                    }
                  }}
                  className="w-full px-3 py-2 border border-input rounded-md focus:outline-none focus:ring-2 focus:ring-ring focus:border-transparent"
                  placeholder="Enter meeting title"
                  autoFocus
                />
              </div>
            </div>
          </div>
          <DialogFooter>
            <button
              onClick={handleEditCancel}
              className="px-4 py-2 text-sm font-medium text-foreground bg-secondary hover:bg-muted rounded-md transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleEditConfirm}
              className="px-4 py-2 text-sm font-medium text-primary-foreground bg-primary hover:bg-primary/90 rounded-md transition-colors"
            >
              Save
            </button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
};

export default Sidebar;

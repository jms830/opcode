import React, { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { api, type ClaudeInstallation, type ShellConfig, type AvailableShells } from "@/lib/api";
import { cn } from "@/lib/utils";
import { CheckCircle, HardDrive, Settings, Terminal, Info, ChevronDown, ChevronRight, Loader2, Check } from "lucide-react";

interface ClaudeVersionSelectorProps {
  /**
   * Currently selected installation path
   */
  selectedPath?: string | null;
  /**
   * Callback when an installation is selected
   */
  onSelect: (installation: ClaudeInstallation) => void;
  /**
   * Optional className for styling
   */
  className?: string;
  /**
   * Whether to show the save button
   */
  showSaveButton?: boolean;
  /**
   * Callback when save is clicked
   */
  onSave?: () => void;
  /**
   * Whether save is in progress
   */
  isSaving?: boolean;
  /**
   * Simplified mode for cleaner UI
   */
  simplified?: boolean;
  /**
   * Callback when shell config changes (for parent to track changes)
   */
  onShellConfigChange?: (config: ShellConfig, hasChanges: boolean) => void;
  /**
   * Initial shell config (if loading from settings)
   */
  initialShellConfig?: ShellConfig | null;
}

/**
 * ClaudeVersionSelector component for selecting Claude Code installations
 * Supports system installations and user preferences
 * 
 * @example
 * <ClaudeVersionSelector
 *   selectedPath={currentPath}
 *   onSelect={(installation) => setSelectedInstallation(installation)}
 * />
 */
export const ClaudeVersionSelector: React.FC<ClaudeVersionSelectorProps> = ({
  selectedPath,
  onSelect,
  className,
  showSaveButton = false,
  onSave,
  isSaving = false,
  simplified = false,
  onShellConfigChange,
  initialShellConfig,
}) => {
  const [installations, setInstallations] = useState<ClaudeInstallation[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedInstallation, setSelectedInstallation] = useState<ClaudeInstallation | null>(null);
  
  // Advanced settings state
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [shellConfig, setShellConfig] = useState<ShellConfig | null>(initialShellConfig || null);
  const [availableShells, setAvailableShells] = useState<AvailableShells | null>(null);
  const [detectingWslClaude, setDetectingWslClaude] = useState(false);
  const [isWindows, setIsWindows] = useState(false);

  useEffect(() => {
    loadInstallations();
    loadShellSettings();
  }, []);

  useEffect(() => {
    // Update selected installation when selectedPath changes
    if (selectedPath && installations.length > 0) {
      const found = installations.find(i => i.path === selectedPath);
      if (found) {
        setSelectedInstallation(found);
      }
    }
  }, [selectedPath, installations]);

  const loadInstallations = async () => {
    try {
      setLoading(true);
      setError(null);
      const foundInstallations = await api.listClaudeInstallations();
      setInstallations(foundInstallations);
      
      // If we have a selected path, find and select it
      if (selectedPath) {
        const found = foundInstallations.find(i => i.path === selectedPath);
        if (found) {
          setSelectedInstallation(found);
        }
      } else if (foundInstallations.length > 0) {
        // Auto-select the first (best) installation
        setSelectedInstallation(foundInstallations[0]);
        onSelect(foundInstallations[0]);
      }
    } catch (err) {
      console.error("Failed to load Claude installations:", err);
      setError(err instanceof Error ? err.message : "Failed to load Claude installations");
    } finally {
      setLoading(false);
    }
  };
  
  const loadShellSettings = async () => {
    try {
      const shells = await api.getAvailableShells();
      setAvailableShells(shells);
      
      // Detect if we're on Windows
      const onWindows = shells.wsl_distributions.length > 0 || !!shells.git_bash_path;
      setIsWindows(onWindows);
      
      // Load current shell config if not provided
      if (!initialShellConfig) {
        const config = await api.getShellConfig();
        setShellConfig(config);
      }
    } catch (err) {
      console.error("Failed to load shell settings:", err);
    }
  };
  
  const updateShellConfig = (updates: Partial<ShellConfig>) => {
    const newConfig = shellConfig 
      ? { ...shellConfig, ...updates } 
      : { environment: 'native' as const, ...updates };
    setShellConfig(newConfig);
    onShellConfigChange?.(newConfig, true);
  };
  
  const handleAutoDetectWslClaude = async () => {
    if (!shellConfig?.wsl_distro) return;
    
    setDetectingWslClaude(true);
    try {
      const result = await api.autoDetectWslClaude(shellConfig.wsl_distro);
      if (result && result.wsl_claude_path) {
        updateShellConfig({ wsl_claude_path: result.wsl_claude_path });
      }
    } catch (err) {
      console.error("Failed to auto-detect Claude in WSL:", err);
    } finally {
      setDetectingWslClaude(false);
    }
  };

  const handleInstallationChange = (installationPath: string) => {
    const installation = installations.find(i => i.path === installationPath);
    if (installation) {
      setSelectedInstallation(installation);
      onSelect(installation);
    }
  };

  const getInstallationIcon = (installation: ClaudeInstallation) => {
    switch (installation.installation_type) {
      case "System":
        return <HardDrive className="h-4 w-4" />;
      case "Custom":
        return <Settings className="h-4 w-4" />;
      default:
        return <HardDrive className="h-4 w-4" />;
    }
  };

  const getInstallationTypeColor = (installation: ClaudeInstallation) => {
    switch (installation.installation_type) {
      case "System":
        return "default";
      case "Custom":
        return "secondary";
      default:
        return "outline";
    }
  };

  if (loading) {
    if (simplified) {
      return (
        <div className="space-y-2">
          <Label className="text-sm font-medium">Claude Installation</Label>
          <div className="flex items-center justify-center py-3 border rounded-lg">
            <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-primary"></div>
          </div>
        </div>
      );
    }
    return (
      <Card className={className}>
        <CardHeader>
          <CardTitle>Claude Code Installation</CardTitle>
          <CardDescription>Loading available installations...</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-center py-4">
            <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-primary"></div>
          </div>
        </CardContent>
      </Card>
    );
  }

  if (error) {
    if (simplified) {
      return (
        <div className="space-y-2">
          <Label className="text-sm font-medium">Claude Installation</Label>
          <div className="p-3 border border-destructive/50 rounded-lg bg-destructive/10">
            <p className="text-sm text-destructive mb-2">{error}</p>
            <Button onClick={loadInstallations} variant="outline" size="sm">
              Retry
            </Button>
          </div>
        </div>
      );
    }
    return (
      <Card className={className}>
        <CardHeader>
          <CardTitle>Claude Code Installation</CardTitle>
          <CardDescription>Error loading installations</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="text-sm text-destructive mb-4">{error}</div>
          <Button onClick={loadInstallations} variant="outline" size="sm">
            Retry
          </Button>
        </CardContent>
      </Card>
    );
  }

  const systemInstallations = installations.filter(i => i.installation_type === "System");
  const customInstallations = installations.filter(i => i.installation_type === "Custom");

  // Simplified mode - more streamlined UI
  if (simplified) {
    return (
      <div className={cn("space-y-3", className)}>
        <div className="flex items-center justify-between">
          <div className="space-y-0.5">
            <Label htmlFor="claude-installation" className="text-sm font-medium">Claude Installation</Label>
            <p className="text-xs text-muted-foreground">
              Select which version of Claude to use
            </p>
          </div>
          {selectedInstallation && (
            <Badge variant={getInstallationTypeColor(selectedInstallation)} className="text-xs">
              {selectedInstallation.installation_type}
            </Badge>
          )}
        </div>
        
        <Select value={selectedInstallation?.path || ""} onValueChange={handleInstallationChange}>
          <SelectTrigger id="claude-installation" className="w-full">
            <SelectValue placeholder="Choose Claude installation">
              {selectedInstallation && (
                <div className="flex items-center gap-2">
                  <Terminal className="h-3.5 w-3.5 text-muted-foreground" />
                  <span className="font-mono text-sm">{selectedInstallation.path.split('/').pop() || selectedInstallation.path}</span>
                  {selectedInstallation.version && (
                    <span className="text-xs text-muted-foreground">({selectedInstallation.version})</span>
                  )}
                </div>
              )}
            </SelectValue>
          </SelectTrigger>
          <SelectContent side="bottom" align="start" sideOffset={5}>
            {installations.length === 0 ? (
              <div className="p-4 text-center text-sm text-muted-foreground">
                No Claude installations found
              </div>
            ) : (
              <>
                {installations.map((installation) => (
                  <SelectItem key={installation.path} value={installation.path} className="cursor-pointer hover:bg-accent focus:bg-accent">
                    <div className="flex items-center gap-2 py-1">
                      <Terminal className="h-3.5 w-3.5 text-muted-foreground" />
                      <div className="flex-1">
                        <div className="font-mono text-sm">{installation.path}</div>
                        <div className="flex items-center gap-2 text-xs text-muted-foreground">
                          <span>{installation.version || "Unknown version"}</span>
                          <span>•</span>
                          <span>{installation.source}</span>
                          <Badge variant={getInstallationTypeColor(installation)} className="text-xs ml-2">
                            {installation.installation_type}
                          </Badge>
                        </div>
                      </div>
                    </div>
                  </SelectItem>
                ))}
              </>
            )}
          </SelectContent>
        </Select>
        
        {selectedInstallation && (
          <div className="flex items-start gap-2 p-2 bg-muted/50 rounded-md">
            <Info className="h-3.5 w-3.5 text-muted-foreground mt-0.5" />
            <div className="text-xs text-muted-foreground">
              <span className="font-medium">Path:</span> <code className="font-mono">{selectedInstallation.path}</code>
              {selectedInstallation.wsl_distro && (
                <span className="ml-2">
                  <Badge variant="outline" className="text-xs">WSL: {selectedInstallation.wsl_distro}</Badge>
                </span>
              )}
            </div>
          </div>
        )}
        
        {/* Advanced Settings (Windows only) */}
        {isWindows && (
          <div className="pt-2">
            <button
              onClick={() => setShowAdvanced(!showAdvanced)}
              className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
            >
              {showAdvanced ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
              Advanced Settings
            </button>
            
            {showAdvanced && (
              <div className="mt-3 p-3 border rounded-lg space-y-4 bg-muted/30">
                <div>
                  <p className="text-xs text-muted-foreground mb-3">
                    Configure how opcode runs Claude Code on Windows. Override auto-detected settings if needed.
                  </p>
                </div>
                
                {/* Shell Environment Selector */}
                <div className="space-y-2">
                  <Label className="text-xs">Shell Environment</Label>
                  <div className="flex items-center gap-1 p-1 bg-muted/50 rounded-md w-fit">
                    <button
                      onClick={() => updateShellConfig({ environment: 'native' })}
                      className={cn(
                        "flex items-center gap-1 px-3 py-1.5 text-xs font-medium rounded transition-all",
                        shellConfig?.environment === 'native' 
                          ? "bg-background shadow-sm" 
                          : "hover:bg-background/50"
                      )}
                    >
                      {shellConfig?.environment === 'native' && <Check className="h-3 w-3" />}
                      Native
                    </button>
                    {availableShells && availableShells.wsl_distributions.length > 0 && (
                      <button
                        onClick={() => {
                          const defaultDistro = availableShells.wsl_distributions.find(d => d.is_default)?.name 
                            || availableShells.wsl_distributions[0]?.name;
                          updateShellConfig({ environment: 'wsl', wsl_distro: defaultDistro });
                        }}
                        className={cn(
                          "flex items-center gap-1 px-3 py-1.5 text-xs font-medium rounded transition-all",
                          shellConfig?.environment === 'wsl' 
                            ? "bg-background shadow-sm" 
                            : "hover:bg-background/50"
                        )}
                      >
                        {shellConfig?.environment === 'wsl' && <Check className="h-3 w-3" />}
                        WSL
                      </button>
                    )}
                    {availableShells?.git_bash_path && (
                      <button
                        onClick={() => updateShellConfig({ environment: 'gitbash', git_bash_path: availableShells.git_bash_path })}
                        className={cn(
                          "flex items-center gap-1 px-3 py-1.5 text-xs font-medium rounded transition-all",
                          shellConfig?.environment === 'gitbash' 
                            ? "bg-background shadow-sm" 
                            : "hover:bg-background/50"
                        )}
                      >
                        {shellConfig?.environment === 'gitbash' && <Check className="h-3 w-3" />}
                        Git Bash
                      </button>
                    )}
                  </div>
                </div>
                
                {/* WSL Distribution Selector */}
                {shellConfig?.environment === 'wsl' && availableShells && availableShells.wsl_distributions.length > 0 && (
                  <div className="space-y-2">
                    <Label className="text-xs">WSL Distribution</Label>
                    <select
                      value={shellConfig.wsl_distro || ''}
                      onChange={(e) => updateShellConfig({ wsl_distro: e.target.value })}
                      className="w-full px-2 py-1.5 rounded border border-input bg-background text-xs"
                    >
                      {availableShells.wsl_distributions.map((distro) => (
                        <option key={distro.name} value={distro.name}>
                          {distro.name} {distro.is_default ? '(default)' : ''} {distro.version ? `- WSL${distro.version}` : ''}
                        </option>
                      ))}
                    </select>
                  </div>
                )}
                
                {/* WSL Claude Path */}
                {shellConfig?.environment === 'wsl' && (
                  <div className="space-y-2">
                    <div className="flex items-center justify-between">
                      <Label className="text-xs">Claude Path in WSL</Label>
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={handleAutoDetectWslClaude}
                        disabled={detectingWslClaude}
                        className="h-6 text-xs px-2"
                      >
                        {detectingWslClaude ? (
                          <>
                            <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                            Detecting...
                          </>
                        ) : (
                          'Auto-detect'
                        )}
                      </Button>
                    </div>
                    <Input
                      placeholder="/home/user/.nvm/versions/node/v20/bin/claude"
                      value={shellConfig.wsl_claude_path || ''}
                      onChange={(e) => updateShellConfig({ wsl_claude_path: e.target.value })}
                      className="text-xs h-8"
                    />
                    {shellConfig.wsl_claude_path && (
                      <p className="text-xs text-green-600 dark:text-green-400 flex items-center gap-1">
                        <Check className="h-3 w-3" />
                        Claude path configured
                      </p>
                    )}
                  </div>
                )}
                
                {/* Help text */}
                <div className="pt-2 border-t border-border/50">
                  <p className="text-xs text-muted-foreground">
                    <strong>Tips:</strong> Native uses Windows-installed Claude. WSL uses Claude inside your Linux distribution. 
                    WSL installations are auto-detected in the dropdown above.
                  </p>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    );
  }

  // Original card-based UI
  return (
    <Card className={className}>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <CheckCircle className="h-5 w-5" />
          Claude Code Installation
        </CardTitle>
        <CardDescription>
          Choose your preferred Claude Code installation.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        {/* Available Installations */}
        <div className="space-y-3">
          <Label className="text-sm font-medium">Available Installations</Label>
          <Select value={selectedInstallation?.path || ""} onValueChange={handleInstallationChange}>
            <SelectTrigger>
              <SelectValue placeholder="Select Claude installation">
                {selectedInstallation && (
                  <div className="flex items-center gap-2">
                    {getInstallationIcon(selectedInstallation)}
                    <span className="truncate">{selectedInstallation.path}</span>
                    <Badge variant={getInstallationTypeColor(selectedInstallation)} className="text-xs">
                      {selectedInstallation.installation_type}
                    </Badge>
                  </div>
                )}
              </SelectValue>
            </SelectTrigger>
            <SelectContent side="bottom" align="start" sideOffset={5}>
              {systemInstallations.length > 0 && (
                <>
                  <div className="px-2 py-1.5 text-xs font-semibold text-muted-foreground">System Installations</div>
                  {systemInstallations.map((installation) => (
                    <SelectItem key={installation.path} value={installation.path} className="cursor-pointer hover:bg-accent focus:bg-accent">
                      <div className="flex items-center gap-2 w-full">
                        {getInstallationIcon(installation)}
                        <div className="flex-1 min-w-0">
                          <div className="font-medium truncate">{installation.path}</div>
                          <div className="text-xs text-muted-foreground">
                            {installation.version || "Version unknown"} • {installation.source}
                          </div>
                        </div>
                        <Badge variant="outline" className="text-xs">
                          System
                        </Badge>
                      </div>
                    </SelectItem>
                  ))}
                </>
              )}

              {customInstallations.length > 0 && (
                <>
                  <div className="px-2 py-1.5 text-xs font-semibold text-muted-foreground">Custom Installations</div>
                  {customInstallations.map((installation) => (
                    <SelectItem key={installation.path} value={installation.path} className="cursor-pointer hover:bg-accent focus:bg-accent">
                      <div className="flex items-center gap-2 w-full">
                        {getInstallationIcon(installation)}
                        <div className="flex-1 min-w-0">
                          <div className="font-medium truncate">{installation.path}</div>
                          <div className="text-xs text-muted-foreground">
                            {installation.version || "Version unknown"} • {installation.source}
                          </div>
                        </div>
                        <Badge variant="outline" className="text-xs">
                          Custom
                        </Badge>
                      </div>
                    </SelectItem>
                  ))}
                </>
              )}
            </SelectContent>
          </Select>
        </div>

        {/* Installation Details */}
        {selectedInstallation && (
          <div className="p-3 bg-muted rounded-lg space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium">Selected Installation</span>
              <Badge variant={getInstallationTypeColor(selectedInstallation)} className="text-xs">
                {selectedInstallation.installation_type}
              </Badge>
            </div>
            <div className="text-sm text-muted-foreground">
              <div><strong>Path:</strong> {selectedInstallation.path}</div>
              <div><strong>Source:</strong> {selectedInstallation.source}</div>
              {selectedInstallation.version && (
                <div><strong>Version:</strong> {selectedInstallation.version}</div>
              )}
            </div>
          </div>
        )}

        {/* Save Button */}
        {showSaveButton && (
          <Button 
            onClick={onSave} 
            disabled={isSaving || !selectedInstallation}
            className="w-full"
          >
            {isSaving ? "Saving..." : "Save Selection"}
          </Button>
        )}
      </CardContent>
    </Card>
  );
}; 

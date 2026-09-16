"use client"

import React from "react"
import { FlaskConical, AlertCircle } from "lucide-react"
import CopilotSettings from "./copilot/CopilotSettings"
import ModesSettings from "./copilot/ModesSettings"

/**
 * A pre-gate feature rendered as a Beta card. The copilot has not passed the
 * I9 gate (A5 GO + a live smoke against a real provider), so its controls live
 * here, under the yellow banner, rather than as a top-level Settings tab.
 *
 * This is also the ONLY place these two components are mounted. v1.2.0 shipped
 * them inside `SettingTabs.tsx`, a component nothing renders, so the switch
 * that turns the copilot on could not be reached from the app at all
 * (ADR-0045). `BetaSettings.test.tsx` asserts they are mounted from here.
 */
function BetaCard({ title, description, children }: { title: string; description: string; children: React.ReactNode }) {
  return (
    <div className="bg-card rounded-lg border border-border p-6 shadow-sm">
      <div className="flex items-center gap-2 mb-2">
        <FlaskConical className="h-5 w-5 text-muted-foreground" />
        <h3 className="text-lg font-semibold text-foreground">{title}</h3>
        <span className="px-2 py-0.5 text-xs font-medium bg-yellow-100 text-yellow-800 rounded-full">
          BETA
        </span>
      </div>
      <p className="text-sm text-muted-foreground mb-4">{description}</p>
      {children}
    </div>
  );
}

export function BetaSettings() {

  return (
    <div className="space-y-6">
      {/* Yellow Warning Banner */}
      <div className="flex items-start gap-3 p-4 bg-yellow-50 border border-yellow-200 rounded-lg">
        <AlertCircle className="h-5 w-5 text-yellow-600 flex-shrink-0 mt-0.5" />
        <div className="text-sm text-yellow-800">
          <p className="font-medium">Beta Features</p>
          <p className="mt-1">
            These features are still being tested. You may encounter issues, and we appreciate your feedback.
          </p>
        </div>
      </div>

      {/* Live copilot (BACKLOG EPIC I). Off by default until the I9 gate; the
          switch is inside the card. */}
      <BetaCard
        title="Live copilot"
        description="An always-on-top panel that follows a recording you start and answers from the last few minutes of the conversation, on this device. Every answer is a draft tied to the transcript segment it came from. Nothing here has been validated against a real model yet."
      >
        <CopilotSettings />
      </BetaCard>

      <BetaCard
        title="Meeting modes"
        description="What kind of conversation the copilot thinks it is in: who you are, who the other side is, which actions it offers and how strictly it must cite. Built-ins are read-only; a custom mode's wording can be edited, its permissions cannot."
      >
        <ModesSettings />
      </BetaCard>

      {/* Info Box */}
      <div className="p-4 bg-accent border border-primary/20 rounded-lg">
        <p className="text-sm text-primary">
          <strong>Note:</strong> When disabled, beta features will be hidden. Your existing meetings remain unaffected.
        </p>
      </div>
    </div>
  );
}

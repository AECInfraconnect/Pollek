import React from 'react';
import { RoutingPreviewPanel } from './RoutingPreviewPanel';
import { AgentDeploymentTimeline } from './AgentDeploymentTimeline';
import type { DeploymentSession, DeploymentEvent } from '../../types/deployment';

interface Props {
  session: DeploymentSession;
  events: DeploymentEvent[];
  onDeploy: () => void;
  onCancel: () => void;
}

export const PolicyDeploymentWizard: React.FC<Props> = ({ session, events, onDeploy, onCancel }) => {
  const isDraft = session.status === 'draft';
  const isPlanning = session.status === 'planning';
  const isActive = session.status === 'active' || session.status === 'partially_active';

  return (
    <div className="max-w-4xl mx-auto py-8">
      <div className="mb-8 flex justify-between items-center">
        <div>
          <h2 className="text-2xl font-bold text-gray-900 dark:text-white">Deployment Wizard</h2>
          <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">Policy: {session.policy_id}</p>
        </div>
        <div className="flex space-x-3">
          <button
            onClick={onCancel}
            className="px-4 py-2 bg-white text-gray-700 border border-gray-300 rounded-md hover:bg-gray-50 dark:bg-gray-800 dark:text-gray-300 dark:border-gray-600 dark:hover:bg-gray-700"
          >
            Cancel
          </button>
          {!isActive && (
            <button
              onClick={onDeploy}
              disabled={isPlanning}
              className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {isDraft ? 'Start Deployment' : 'Deploy'}
            </button>
          )}
        </div>
      </div>

      <div className="space-y-8">
        {session.routing_plan && (
          <RoutingPreviewPanel plan={session.routing_plan} />
        )}

        <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-6 mt-8">
          <AgentDeploymentTimeline events={events} />
        </div>
      </div>
    </div>
  );
};

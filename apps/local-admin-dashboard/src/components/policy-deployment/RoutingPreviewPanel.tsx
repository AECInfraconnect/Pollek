import React from 'react';
import type { RoutingPlan } from '../../types/deployment';

interface Props {
  plan: RoutingPlan;
}

export const RoutingPreviewPanel: React.FC<Props> = ({ plan }) => {
  return (
    <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-6">
      <h3 className="text-lg font-medium text-gray-900 dark:text-gray-100 mb-4">Routing Plan Preview</h3>
      
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="p-4 bg-gray-50 dark:bg-gray-700 rounded-md border border-gray-200 dark:border-gray-600">
          <h4 className="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-2">Selected PEP Layer</h4>
          <p className="text-md font-medium text-gray-900 dark:text-white">{plan.selected_pep.name.en}</p>
          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1 font-mono">{plan.selected_pep.layer}</p>
        </div>

        <div className="p-4 bg-gray-50 dark:bg-gray-700 rounded-md border border-gray-200 dark:border-gray-600">
          <h4 className="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-2">Selected PDP Engine</h4>
          <p className="text-md font-medium text-gray-900 dark:text-white">{plan.selected_pdp.engine}</p>
        </div>
      </div>

      {plan.fallback_pep && (
        <div className="mt-4 p-4 bg-yellow-50 dark:bg-yellow-900/30 rounded-md border border-yellow-200 dark:border-yellow-700">
          <h4 className="text-sm font-semibold text-yellow-800 dark:text-yellow-500 uppercase tracking-wider mb-2">Fallback PEP Layer</h4>
          <p className="text-md font-medium text-yellow-900 dark:text-yellow-400">{plan.fallback_pep.name.en}</p>
          <p className="text-sm text-yellow-700 dark:text-yellow-600 mt-1 font-mono">{plan.fallback_pep.layer}</p>
        </div>
      )}
    </div>
  );
};

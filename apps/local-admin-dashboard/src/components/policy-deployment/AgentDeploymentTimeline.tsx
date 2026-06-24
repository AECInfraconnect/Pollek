import React from 'react';
import type { DeploymentEvent } from '../../types/deployment';

interface Props {
  events: DeploymentEvent[];
}

export const AgentDeploymentTimeline: React.FC<Props> = ({ events }) => {
  return (
    <div className="flex flex-col space-y-4">
      <h3 className="text-lg font-semibold text-gray-800 dark:text-gray-100">Deployment Timeline</h3>
      <div className="relative border-l border-gray-200 dark:border-gray-700 ml-3">
        {events.map((event) => (
          <div key={event.event_id} className="mb-6 ml-6">
            <span
              className={`absolute flex items-center justify-center w-6 h-6 rounded-full -left-3 ring-8 ring-white dark:ring-gray-900 ${
                event.status === 'success' ? 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-300' :
                event.status === 'error' ? 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300' :
                event.status === 'warning' ? 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300' :
                'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300'
              }`}
            >
              <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
                {event.status === 'success' ? (
                  <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                ) : event.status === 'error' ? (
                  <path fillRule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clipRule="evenodd" />
                ) : (
                  <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clipRule="evenodd" />
                )}
              </svg>
            </span>
            <h4 className="flex items-center mb-1 text-sm font-semibold text-gray-900 dark:text-white">
              {event.title.en}
              {event.phase && (
                <span className="bg-blue-100 text-blue-800 text-xs font-medium mr-2 px-2.5 py-0.5 rounded dark:bg-blue-900 dark:text-blue-300 ml-3">
                  {event.phase}
                </span>
              )}
            </h4>
            <time className="block mb-2 text-xs font-normal leading-none text-gray-400 dark:text-gray-500">
              {new Date(event.created_at).toLocaleString()}
            </time>
            <p className="mb-4 text-sm font-normal text-gray-500 dark:text-gray-400">
              {event.detail.en}
            </p>
            {event.technical_detail && (
              <pre className="text-xs bg-gray-100 dark:bg-gray-800 p-2 rounded text-gray-600 dark:text-gray-300 mb-2">
                {event.technical_detail}
              </pre>
            )}
            {event.user_action && (
              <a
                href={event.user_action.action_url}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center px-4 py-2 text-sm font-medium text-gray-900 bg-white border border-gray-200 rounded-lg hover:bg-gray-100 hover:text-blue-700 focus:z-10 focus:ring-4 focus:outline-none focus:ring-gray-100 focus:text-blue-700 dark:bg-gray-800 dark:text-gray-400 dark:border-gray-600 dark:hover:text-white dark:hover:bg-gray-700 dark:focus:ring-gray-700"
              >
                Action Required: {event.user_action.kind}
              </a>
            )}
          </div>
        ))}
      </div>
    </div>
  );
};

import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  tutorialSidebar: [
    'intro',
    {
      type: 'category',
      label: 'Getting Started',
      items: [
        'getting-started/installation',
        'getting-started/quick-start',
        'getting-started/first-backup',
      ],
    },
    {
      type: 'category',
      label: 'Core Concepts',
      items: [
        'concepts/architecture',
        'concepts/traits',
        'concepts/backends',
        'concepts/capabilities',
        'concepts/plans',
        'concepts/error-handling',
      ],
    },
    {
      type: 'category',
      label: 'Storage Providers',
      items: [
        'providers/btrfs',
        'providers/lvm',
        'providers/zfs',
        'providers/vss',
      ],
    },
    {
      type: 'category',
      label: 'CLI Reference',
      items: [
        'cli/overview',
        'cli/snapshot',
        'cli/backup',
        'cli/restore',
      ],
    },
    {
      type: 'category',
      label: 'API Reference',
      items: [
        'api/backend',
        'api/snapshot-provider',
        'api/backup-executor',
        'api/restore-planner',
        'api/types',
        'api/errors',
      ],
    },
    {
      type: 'category',
      label: 'Testing',
      items: [
        'testing/overview',
        'testing/integration-tests',
      ],
    },
  ],
};

export default sidebars;

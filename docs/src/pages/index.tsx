import React from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import styles from './index.module.css';

function HomepageHeader() {
  return (
    <header className={styles.hero}>
      <div className={styles.heroInner}>
        <h1 className={styles.heroTitle}>vpt-rs</h1>
        <p className={styles.heroSubtitle}>
          Cross-platform volume backup library and CLI tool written in Rust
        </p>
        <div className={styles.heroBadges}>
          <span className={styles.badge}>Btrfs</span>
          <span className={styles.badge}>LVM</span>
          <span className={styles.badge}>ZFS</span>
          <span className={styles.badge}>Windows VSS</span>
        </div>
        <div className={styles.heroButtons}>
          <Link className={styles.button} to="/getting-started/installation">
            Get Started
          </Link>
          <Link className={styles.buttonOutline} to="/concepts/architecture">
            Architecture
          </Link>
        </div>
      </div>
    </header>
  );
}

function FeatureSection() {
  const features = [
    {
      title: 'Unified API',
      description: 'One set of traits (SnapshotProvider, BackupExecutor, RestorePlanner) works across all storage backends.',
      icon: '🔧',
    },
    {
      title: 'Plan-then-Execute',
      description: 'Plans are validated before execution. Unit test your backup strategy without running privileged commands.',
      icon: '📋',
    },
    {
      title: 'Incremental Backup',
      description: 'Btrfs and ZFS support incremental send/receive with parent snapshot references for efficient delta backups.',
      icon: '⚡',
    },
    {
      title: 'Cross-Platform',
      description: 'Linux (Btrfs, LVM, ZFS) and Windows (VSS) fully implemented. macOS and Unix stubs ready for future backends.',
      icon: '🌐',
    },
  ];

  return (
    <section className={styles.features}>
      <div className={styles.featuresGrid}>
        {features.map((f, i) => (
          <div key={i} className={styles.featureCard}>
            <div className={styles.featureIcon}>{f.icon}</div>
            <h3>{f.title}</h3>
            <p>{f.description}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

export default function Home() {
  return (
    <Layout title="Home" description="Cross-platform volume backup library and CLI">
      <HomepageHeader />
      <FeatureSection />
    </Layout>
  );
}

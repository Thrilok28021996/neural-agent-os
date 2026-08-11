import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'

type AssistantState = 'idle' | 'listening' | 'speaking' | 'thinking'

interface ThreeDAssistantProps {
  state: AssistantState
  size?: number
  onStateChange?: (state: AssistantState) => void
}

export function ThreeDAssistant({ state, size = 280, onStateChange }: ThreeDAssistantProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const sceneRef = useRef<{ core: THREE.Mesh; ring1: THREE.Mesh; ring2: THREE.Mesh; particles: THREE.Points; glow: THREE.Mesh } | null>(null)
  const frameRef = useRef<number>(0)
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    const container = containerRef.current
    if (!container || mounted) return

    const scene = new THREE.Scene()
    const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 100)
    camera.position.z = 6

    const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true })
    renderer.setSize(size, size)
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    container.appendChild(renderer.domElement)

    // Lighting
    scene.add(new THREE.AmbientLight(0x443366, 2))
    const pointLight = new THREE.PointLight(0xbb9cff, 8, 15)
    pointLight.position.set(2, 1, 4)
    scene.add(pointLight)
    const pointLight2 = new THREE.PointLight(0x7ae2b0, 3, 10)
    pointLight2.position.set(-2, -1, 3)
    scene.add(pointLight2)

    // Core sphere
    const coreGeom = new THREE.SphereGeometry(0.7, 48, 48)
    const coreMat = new THREE.MeshPhysicalMaterial({
      color: 0xbb9cff,
      emissive: 0x332244,
      roughness: 0.2,
      metalness: 0.1,
      clearcoat: 0.3,
    })
    const core = new THREE.Mesh(coreGeom, coreMat)
    scene.add(core)

    // Orbital rings
    const ringGeom1 = new THREE.TorusGeometry(1.3, 0.025, 32, 120)
    const ringMat1 = new THREE.MeshStandardMaterial({ color: 0x9b7cd4, emissive: 0x221133, roughness: 0.3, metalness: 0.5 })
    const ring1 = new THREE.Mesh(ringGeom1, ringMat1)
    ring1.rotation.x = Math.PI / 2.8
    ring1.rotation.y = Math.PI / 5
    scene.add(ring1)

    const ringGeom2 = new THREE.TorusGeometry(1.65, 0.02, 24, 100)
    const ringMat2 = new THREE.MeshStandardMaterial({ color: 0x7ae2b0, emissive: 0x112211, roughness: 0.4, metalness: 0.3 })
    const ring2 = new THREE.Mesh(ringGeom2, ringMat2)
    ring2.rotation.x = -Math.PI / 3
    ring2.rotation.y = -Math.PI / 4
    scene.add(ring2)

    // Particles
    const particlesGeom = new THREE.BufferGeometry()
    const particleCount = 200
    const positions = new Float32Array(particleCount * 3)
    for (let i = 0; i < particleCount * 3; i += 3) {
      const r = 1.8 + Math.random() * 1.2
      const theta = Math.random() * Math.PI * 2
      const phi = Math.random() * Math.PI
      positions[i] = r * Math.sin(phi) * Math.cos(theta)
      positions[i + 1] = r * Math.sin(phi) * Math.sin(theta)
      positions[i + 2] = r * Math.cos(phi)
    }
    particlesGeom.setAttribute('position', new THREE.BufferAttribute(positions, 3))
    const particlesMat = new THREE.PointsMaterial({ color: 0xbb9cff, size: 0.03, transparent: true, opacity: 0.7 })
    const particles = new THREE.Points(particlesGeom, particlesMat)
    scene.add(particles)

    // Outer glow
    const glowGeom = new THREE.SphereGeometry(1.0, 32, 32)
    const glowMat = new THREE.MeshBasicMaterial({ color: 0xbb9cff, transparent: true, opacity: 0.08 })
    const glow = new THREE.Mesh(glowGeom, glowMat)
    scene.add(glow)

    sceneRef.current = { core, ring1, ring2, particles, glow }
    setMounted(true)

    // Animation loop
    const animate = () => {
      frameRef.current = requestAnimationFrame(animate)
      const time = Date.now() * 0.001

      if (sceneRef.current) {
        const { core, ring1, ring2, particles, glow } = sceneRef.current
        core.rotation.y += 0.005
        core.rotation.x += 0.002
        ring1.rotation.z += 0.003
        ring2.rotation.z -= 0.004
        particles.rotation.y += 0.001
        particles.rotation.x += 0.0005
        glow.rotation.y += 0.002

        // State-based animations
        const pulse = Math.sin(time * 3) * 0.5 + 0.5
        switch (state) {
          case 'listening':
            core.scale.setScalar(1 + pulse * 0.15)
            glow.scale.setScalar(1 + pulse * 0.4)
            if (!Array.isArray(particles.material)) particles.material.opacity = 0.5 + pulse * 0.4
            break
          case 'speaking':
            core.scale.setScalar(1 + pulse * 0.2)
            ring1.scale.setScalar(1 + pulse * 0.1)
            ring2.scale.setScalar(1 + pulse * 0.08)
            glow.scale.setScalar(1 + pulse * 0.6)
            if (!Array.isArray(particles.material)) particles.material.opacity = 0.3 + pulse * 0.6
            break
          case 'thinking':
            ring1.rotation.z += 0.01
            ring2.rotation.z -= 0.01
            particles.rotation.y += 0.005
            glow.scale.setScalar(1 + pulse * 0.2)
            break
          default: // idle
            core.scale.lerp(new THREE.Vector3(1, 1, 1), 0.05)
            glow.scale.lerp(new THREE.Vector3(1, 1, 1), 0.05)
            if (!Array.isArray(particles.material)) particles.material.opacity = 0.7
        }
      }

      renderer.render(scene, camera)
    }
    animate()

    return () => {
      cancelAnimationFrame(frameRef.current)
      renderer.dispose()
      if (container.contains(renderer.domElement)) {
        container.removeChild(renderer.domElement)
      }
    }
  }, [size, mounted])

  // Update state without re-creating scene
  useEffect(() => {
    if (!sceneRef.current) return
    const { core, glow } = sceneRef.current
    const coreMat = core.material as THREE.MeshPhysicalMaterial
    const glowMat = glow.material as THREE.MeshBasicMaterial

    switch (state) {
      case 'listening':
        coreMat.emissive.set(0x443366)
        glowMat.opacity = 0.15
        break
      case 'speaking':
        coreMat.emissive.set(0x336644)
        glowMat.opacity = 0.2
        break
      case 'thinking':
        coreMat.emissive.set(0x443322)
        glowMat.opacity = 0.12
        break
      default:
        coreMat.emissive.set(0x332244)
        glowMat.opacity = 0.08
    }
  }, [state])

  return (
    <div
      ref={containerRef}
      style={{ width: size, height: size, cursor: 'pointer' }}
      onClick={() => onStateChange?.(state === 'idle' ? 'listening' : 'idle')}
      title={`Assistant state: ${state}`}
    />
  )
}

import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'

type AssistantState = 'idle' | 'listening' | 'speaking' | 'thinking'

interface ThreeDAssistantProps {
  state: AssistantState
  size?: number
  onStateChange?: (state: AssistantState) => void
  /** Latest assistant reply, shown in a speech bubble while speaking. */
  speech?: string
}

export function ThreeDAssistant({ state, size = 280, onStateChange, speech }: ThreeDAssistantProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const sceneRef = useRef<{
    core: THREE.Mesh
    face: THREE.Group
    eyes: THREE.Group[]
    mouth: THREE.Mesh
    ring1: THREE.Mesh
    ring2: THREE.Mesh
    particles: THREE.Points
    glow: THREE.Mesh
  } | null>(null)
  const frameRef = useRef<number>(0)
  const stateRef = useRef(state)

  // Keep the animation loop reading the latest state without recreating the
  // WebGL scene (recreating on state change would destroy the canvas).
  useEffect(() => { stateRef.current = state }, [state])

  useEffect(() => {
    const container = containerRef.current
    if (!container) return

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

    // Core body
    const coreGeom = new THREE.SphereGeometry(0.7, 48, 48)
    const coreMat = new THREE.MeshPhysicalMaterial({
      color: 0xbb9cff,
      emissive: 0x332244,
      roughness: 0.2,
      metalness: 0.1,
      clearcoat: 0.3,
    })
    const core = new THREE.Mesh(coreGeom, coreMat)
    core.rotation.x = 0.15
    scene.add(core)

    // Face group (children of core; counter-rotates so it always faces the
    // viewer while the body sways).
    const face = new THREE.Group()
    face.position.z = 0.66
    core.add(face)

    // Eyes
    const eyeMat = new THREE.MeshBasicMaterial({ color: 0x14151c })
    const pupilMat = new THREE.MeshBasicMaterial({ color: 0xd8c9ff })
    const makeEye = (x: number) => {
      const eye = new THREE.Group()
      const socket = new THREE.Mesh(new THREE.SphereGeometry(0.1, 20, 20), eyeMat)
      const pupil = new THREE.Mesh(new THREE.SphereGeometry(0.045, 16, 16), pupilMat)
      pupil.position.z = 0.07
      eye.add(socket)
      eye.add(pupil)
      eye.position.set(x, 0.2, 0)
      face.add(eye)
      return eye
    }
    const eyeL = makeEye(-0.24)
    const eyeR = makeEye(0.24)

    // Mouth (flattened ellipsoid that opens/closes while speaking)
    const mouthGeom = new THREE.SphereGeometry(0.16, 24, 24)
    const mouthMat = new THREE.MeshBasicMaterial({ color: 0x171822 })
    const mouth = new THREE.Mesh(mouthGeom, mouthMat)
    mouth.scale.set(1.1, 0.3, 0.45)
    mouth.position.set(0, -0.22, 0.02)
    face.add(mouth)

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

    sceneRef.current = { core, face, eyes: [eyeL, eyeR], mouth, ring1, ring2, particles, glow }

    // Animation loop: the character moves, blinks, and talks.
    const animate = () => {
      frameRef.current = requestAnimationFrame(animate)
      const time = Date.now() * 0.001

      if (sceneRef.current) {
        const { core, face, eyes, mouth, ring1, ring2, particles, glow } = sceneRef.current

        const currentState = stateRef.current
        // Body motion: gentle sway + bob (faster while speaking).
        core.rotation.y = Math.sin(time * 0.55) * 0.4
        core.rotation.x = 0.15 + Math.sin(time * 0.4) * 0.08
        const bob = Math.sin(time * (currentState === 'speaking' ? 2.4 : 0.9)) * (currentState === 'speaking' ? 0.16 : 0.1)
        core.position.y = bob
        face.rotation.y = -core.rotation.y // keep the face toward the viewer

        ring1.rotation.z += currentState === 'thinking' ? 0.012 : 0.003
        ring2.rotation.z -= currentState === 'thinking' ? 0.012 : 0.004
        particles.rotation.y += currentState === 'thinking' ? 0.005 : 0.001
        particles.rotation.x += 0.0005
        glow.rotation.y += 0.002

        // Blink every ~3.7s for a fraction of a second.
        const blinkPhase = time % 3.7
        const blinking = blinkPhase < 0.12
        const eyeOpen = blinking ? 0.12 : 1
        for (const eye of eyes) {
          eye.scale.y = eyeOpen
          eye.scale.x = blinking ? 1.15 : 1
        }

        const pulse = Math.sin(time * 3) * 0.5 + 0.5
        switch (currentState) {
          case 'listening':
            // Sound-wave feel: body + glow swell, rings breathe.
            core.scale.setScalar(1 + pulse * 0.12)
            glow.scale.setScalar(1 + pulse * 0.4)
            ring1.scale.setScalar(1 + pulse * 0.12)
            ring2.scale.setScalar(1 + pulse * 0.1)
            mouth.scale.y = 0.35 + pulse * 0.25
            for (const eye of eyes) eye.scale.y = 1.12
            if (!Array.isArray(particles.material)) particles.material.opacity = 0.5 + pulse * 0.4
            break
          case 'speaking':
            // Talking: the mouth opens/closes at speech cadence.
            core.scale.setScalar(1 + pulse * 0.1)
            const talk = Math.abs(Math.sin(time * 7.5)) * 0.85 + 0.25
            mouth.scale.y = talk
            mouth.scale.x = 1.05 + pulse * 0.15
            glow.scale.setScalar(1 + pulse * 0.5)
            if (!Array.isArray(particles.material)) particles.material.opacity = 0.3 + pulse * 0.6
            break
          case 'thinking':
            ring1.scale.setScalar(1 + pulse * 0.08)
            ring2.scale.setScalar(1 + pulse * 0.06)
            glow.scale.setScalar(1 + pulse * 0.2)
            mouth.scale.y = 0.2
            if (!Array.isArray(particles.material)) particles.material.opacity = 0.5 + pulse * 0.3
            break
          default: // idle
            core.scale.lerp(new THREE.Vector3(1, 1, 1), 0.05)
            glow.scale.lerp(new THREE.Vector3(1, 1, 1), 0.05)
            ring1.scale.lerp(new THREE.Vector3(1, 1, 1), 0.05)
            ring2.scale.lerp(new THREE.Vector3(1, 1, 1), 0.05)
            mouth.scale.lerp(new THREE.Vector3(1.1, 0.3, 0.45), 0.08)
            if (!Array.isArray(particles.material)) particles.material.opacity = 0.7
        }
      }

      renderer.render(scene, camera)
    }
    animate()

    return () => {
      cancelAnimationFrame(frameRef.current)
      renderer.dispose()
      sceneRef.current = null
      if (container.contains(renderer.domElement)) {
        container.removeChild(renderer.domElement)
      }
    }
  }, [size])

  // Update colors without re-creating the scene
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

  const speaking = state === 'speaking'

  return (
    <div style={{ position: 'relative' }}>
      {speaking && speech && (
        <div className="three-d-speech" role="status" aria-live="polite">{speech}</div>
      )}
      <div
        ref={containerRef}
        style={{ width: size, height: size, cursor: 'pointer' }}
        onClick={() => onStateChange?.(state === 'idle' ? 'listening' : 'idle')}
        title={`Assistant state: ${state}${speaking ? ' · speaking' : ''}`}
      />
    </div>
  )
}

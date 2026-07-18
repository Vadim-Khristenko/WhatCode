package space.vairice.whatcode

import android.graphics.Color
import android.os.Bundle
import android.text.SpannableString
import android.text.Spanned
import android.text.style.ForegroundColorSpan
import android.view.Menu
import android.view.MenuItem
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import com.google.android.material.appbar.MaterialToolbar
import java.io.File
import kotlin.concurrent.thread

/**
 * Простая и удобная UI-обёртка над нативным бинарём WhatCode.
 *
 * Бинарь поставляется в APK как `libwhatcode.so` (единственный способ держать
 * исполняемый файл в приложении: файлы из `nativeLibraryDir` можно запускать).
 * UI — чат: ввод снизу, ответы модели в ленте. Каждый запрос выполняется через
 * одноразовый режим `whatcode --text`, вывод стримится в ленту. Команды `/set`,
 * `/config`, `/persona` работают прямо из строки (переменных окружения на Android
 * нет — конфиг сохраняется в файл настроек).
 */
class MainActivity : AppCompatActivity() {

    private lateinit var output: TextView
    private lateinit var scroll: ScrollView
    private lateinit var binPath: String
    private val teal = Color.parseColor("#39C5BB")

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        setSupportActionBar(findViewById<MaterialToolbar>(R.id.toolbar))

        output = findViewById(R.id.output)
        scroll = findViewById(R.id.scroll)
        val input = findViewById<EditText>(R.id.input)
        val send = findViewById<Button>(R.id.send)

        binPath = File(applicationInfo.nativeLibraryDir, "libwhatcode.so").absolutePath

        printWelcome()

        send.setOnClickListener {
            val prompt = input.text.toString().trim()
            if (prompt.isEmpty()) return@setOnClickListener
            input.text.clear()
            submit(prompt)
        }
    }

    private fun printWelcome() {
        append("WhatCode ${BuildConfig.VERSION_NAME} — UI-обёртка ♪\n", teal)
        append("Меню (⋮): провайдер и ключ, персона, помощь.\n")
        append("Команды в строке: /set, /config, /persona, /workflows.\n\n")
    }

    private fun submit(prompt: String) {
        append("› $prompt\n", teal)
        thread {
            val reply = runWhatCode(prompt)
            runOnUiThread { append("$reply\n\n") }
        }
    }

    override fun onCreateOptionsMenu(menu: Menu): Boolean {
        menuInflater.inflate(R.menu.main, menu)
        return true
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        return when (item.itemId) {
            R.id.action_clear -> {
                output.text = ""
                printWelcome()
                true
            }
            R.id.action_help -> {
                showHelp()
                true
            }
            R.id.action_persona -> {
                showPersonaDialog()
                true
            }
            R.id.action_provider -> {
                showProviderDialog()
                true
            }
            else -> super.onOptionsItemSelected(item)
        }
    }

    private fun showHelp() {
        append(
            "Как пользоваться:\n" +
                "• Введи вопрос — WhatCode ответит (для развёрнутых ответов задай провайдера и ключ).\n" +
                "• Меню (⋮) → «Провайдер и ключ»: выбери провайдера, модель и API-ключ.\n" +
                "• Меню → «Персона»: Мику, Герта, Anis.\n" +
                "• В строке работают команды: /set <ключ> <значение>, /config, /unset <ключ>, /persona <id>.\n" +
                "• Настройки сохраняются на устройстве (переменные окружения на Android не нужны).\n\n",
            teal
        )
    }

    private fun showPersonaDialog() {
        val ids = arrayOf("miku", "herta", "anis", "default")
        val names = arrayOf("Хацунэ Мику", "Великая Герта", "Anis", "Нейтральная")
        AlertDialog.Builder(this)
            .setTitle(getString(R.string.menu_persona))
            .setItems(names) { _, which ->
                runSet("persona", ids[which])
            }
            .setNegativeButton(R.string.dlg_cancel, null)
            .show()
    }

    private fun showProviderDialog() {
        val pad = (16 * resources.displayMetrics.density).toInt()
        val box = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(pad, pad, pad, 0)
        }
        val provider = EditText(this).apply { hint = "provider (anthropic, google_ai, ollama…)" }
        val model = EditText(this).apply { hint = "model (например claude-opus-4-8)" }
        val apiKey = EditText(this).apply { hint = "api_key" }
        box.addView(provider); box.addView(model); box.addView(apiKey)

        AlertDialog.Builder(this)
            .setTitle(getString(R.string.menu_provider))
            .setView(box)
            .setPositiveButton(R.string.dlg_save) { _, _ ->
                val p = provider.text.toString().trim()
                val m = model.text.toString().trim()
                val k = apiKey.text.toString().trim()
                // Провайдер задаём первым: model/api_key применяются к активному.
                if (p.isNotEmpty()) runSet("provider", p)
                if (m.isNotEmpty()) runSet("model", m)
                if (k.isNotEmpty()) runSet("api_key", k)
            }
            .setNegativeButton(R.string.dlg_cancel, null)
            .show()
    }

    /** Выполнить `/set key value` через бинарь (персистентно) и показать ответ. */
    private fun runSet(key: String, value: String) {
        thread {
            val reply = runWhatCode("/set $key $value")
            runOnUiThread { append("$reply\n\n") }
        }
    }

    private fun append(text: String, color: Int? = null) {
        if (color == null) {
            output.append(text)
        } else {
            val span = SpannableString(text)
            span.setSpan(ForegroundColorSpan(color), 0, text.length, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            output.append(span)
        }
        scroll.post { scroll.fullScroll(ScrollView.FOCUS_DOWN) }
    }

    /** Запустить бинарь в одноразовом режиме и вернуть его вывод. */
    private fun runWhatCode(prompt: String): String {
        return try {
            val pb = ProcessBuilder(binPath, "--text", prompt)
            pb.redirectErrorStream(true)
            // Данные приложения — писабельный HOME для настроек/памяти/логов.
            pb.environment()["HOME"] = filesDir.absolutePath
            pb.directory(filesDir)
            val proc = pb.start()
            val out = proc.inputStream.bufferedReader().readText()
            proc.waitFor()
            out.trim().ifEmpty { "(нет вывода)" }
        } catch (e: Exception) {
            "Ошибка запуска WhatCode: ${e.message}"
        }
    }
}
